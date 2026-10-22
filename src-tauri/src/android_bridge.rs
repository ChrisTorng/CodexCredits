//! Thin JNI boundary. Desktop and future Linux never compile this module.
//! Android OS services, secure storage, and the old AlarmManager scheduler remain native.
use jni::objects::{JClass,JObject,JString,JValue};

fn call(method:&str,args:&[&str])->Result<Option<String>,String>{
    let context=ndk_context::android_context();
    let vm=unsafe{jni::JavaVM::from_raw(context.vm().cast())}
        .map_err(|e|format!("JNI VM: {e}"))?;
    let mut env=vm.attach_current_thread().map_err(|e|format!("JNI attach: {e}"))?;
    // Load via application class loader; FindClass fails on native Tokio threads.
    let activity=unsafe{JObject::from_raw(context.context().cast())};
    let loader=env.call_method(&activity,"getClassLoader","()Ljava/lang/ClassLoader;",&[])
        .and_then(|v|v.l()).map_err(|e|format!("ClassLoader: {e}"))?;
    let name=env.new_string("com.christorng.androidcodextools.LegacyBridge")
        .map_err(|e|e.to_string())?;
    let class_obj=env.call_method(&loader,"loadClass","(Ljava/lang/String;)Ljava/lang/Class;",
        &[JValue::Object(&JObject::from(name))]).and_then(|v|v.l())
        .map_err(|e|format!("LegacyBridge class: {e}"))?;
    let class=JClass::from(class_obj);
    let strings=args.iter().map(|v|env.new_string(v).map(JObject::from))
        .collect::<Result<Vec<JObject>,_>>().map_err(|e|e.to_string())?;
    let mut values=vec![JValue::Object(&activity)];
    for s in &strings{values.push(JValue::Object(s));}
    let sig=format!("(Landroid/content/Context;{})Ljava/lang/String;",
        "Ljava/lang/String;".repeat(args.len()));
    // Clear pending Java exceptions before detaching the JNI thread.
    let value=match env.call_static_method(&class,method,&sig,&values)
        .and_then(|v|v.l()){
        Ok(value)=>value,
        Err(error)=>{
            let _=env.exception_clear();
            return Err(format!("Android {method}: {error}"));
        }
    };
    if value.is_null(){return Ok(None);}
    let jstring=JString::from(value);
    let output=env.get_string(&jstring).map_err(|e|e.to_string())?;
    Ok(Some(output.to_string_lossy().into_owned()))
}
pub fn export_legacy()->Result<serde_json::Value,String>{
    serde_json::from_str(&call("exportLegacy",&[])?.ok_or("無舊版資料")?)
        .map_err(|e|format!("舊版設定格式有誤: {e}"))
}
pub fn credential_get(key:&str)->Result<Option<String>,String>{call("readCredential",&[key])}
pub fn credential_put(key:&str,value:&str)->Result<(),String>{
    call("writeCredential",&[key,value]).map(|_|())
}
pub fn credential_remove(key:&str)->Result<(),String>{
    call("deleteCredential",&[key]).map(|_|())
}
pub fn sync_auth(value:&str)->Result<(),String>{call("syncAuth",&[value]).map(|_|())}
pub fn clear_auth()->Result<(),String>{call("clearAuth",&[]).map(|_|())}
pub fn open_url(value:&str)->Result<(),String>{call("openUrl",&[value]).map(|_|())}
pub fn sync_settings(value:&str)->Result<(),String>{call("syncSettings",&[value]).map(|_|())}
pub fn arm_banked(armed:bool)->Result<(),String>{
    call("armBanked",&[if armed{"arm"}else{"cancel"}]).map(|_|())
}
pub fn banked_armed()->Result<bool,String>{
    Ok(call("bankedArmed",&[])?.as_deref()==Some("true"))
}
pub fn platform_status()->Result<serde_json::Value,String>{
    serde_json::from_str(&call("platformStatus",&[])?.ok_or("無 Android 狀態")?).map_err(|e|e.to_string())
}
pub fn open_system_settings(kind:&str)->Result<(),String>{call("openSystemSettings",&[kind]).map(|_|())}
