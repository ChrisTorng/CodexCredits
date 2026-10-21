#!/usr/bin/env python3
"""Stage the original Java Android scheduler as a platform implementation."""
import argparse
from pathlib import Path
import xml.etree.ElementTree as ET

NS="http://schemas.android.com/apk/res/android"
ET.register_namespace("android",NS)
A=lambda field:"{%s}%s"%(NS,field)
p=argparse.ArgumentParser()
p.add_argument("--legacy",required=True,type=Path)
p.add_argument("--release-keystore",type=Path,default=None)
arguments=p.parse_args()
old=arguments.legacy/"app/src/main/java/com/christorng/androidcodextools"
project=Path("src-tauri/gen/android/app")
if not project.is_dir():
    raise SystemExit("Run tauri android init first")
out=project/"src/main/java/com/christorng/androidcodextools"
out.mkdir(parents=True,exist_ok=True)
names=["AlarmReceiver","AndroidNotifications","BackupJobService","BootReceiver",
       "CodexClient","SamplingJobService","ScheduleConfig","Scheduler",
       "SecureStore","UsageHistory"]
for name in names:
    (out/(name+".java")).write_bytes((old/(name+".java")).read_bytes())
(out/"LegacyBridge.java").write_bytes(Path("mobile/android/LegacyBridge.java").read_bytes())
# A pure-Java launcher runs before Tauri and survives failures in Rust startup.
for name in ("StartupTrace", "StartupDiagnosticActivity"):
    (out/(name+".java")).write_bytes(Path("mobile/android"/(name+".java")).read_bytes())


activity=next(project.rglob("MainActivity.kt"),None)
if activity is None:raise SystemExit("Missing generated MainActivity.kt")
original=activity.read_text()
prefix=original.split("class MainActivity",1)[0]
if "import android.os.Bundle" not in prefix:
    prefix+="\nimport android.os.Bundle\n"
activity.write_text(prefix+"""
class MainActivity : TauriActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        StartupTrace.mark(this, "41 tauri-activity-before-super")
        super.onCreate(savedInstanceState)
        StartupTrace.mark(this, "42 tauri-activity-after-super")
        AndroidNotifications.ensureChannel(this)
        StartupTrace.mark(this, "43 notification-channel-ready")
        Scheduler.scheduleQuotaSampling(this)
        StartupTrace.mark(this, "44 native-sampling-ready")
    }
}
""")
manifest=project/"src/main/AndroidManifest.xml"
tree=ET.parse(manifest)
root=tree.getroot()
perms=["INTERNET","ACCESS_NETWORK_STATE","POST_NOTIFICATIONS","RECEIVE_BOOT_COMPLETED",
       "WAKE_LOCK","SCHEDULE_EXACT_ALARM"]
for permission in perms:
    name="android.permission."+permission
    if not any(e.tag=="uses-permission" and e.get(A("name"))==name for e in root):
        ET.SubElement(root,"uses-permission",{A("name"):name})
app=root.find("application")
if app is None:raise SystemExit("Missing application")
app.set(A("allowBackup"),"false")
app.set(A("usesCleartextTraffic"),"true")
# Replace only the icon-launch intent; leave any deep-link intent filters intact.
main_activities = [e for e in app.findall("activity")
    if e.get(A("name"), "").endswith(".MainActivity")]
if len(main_activities) != 1:
    raise SystemExit("Expected exactly one generated MainActivity")
main_activity = main_activities[0]
launcher_count = 0
for intent in list(main_activity.findall("intent-filter")):
    actions = {e.get(A("name")) for e in intent.findall("action")}
    cats = {e.get(A("name")) for e in intent.findall("category")}
    if "android.intent.action.MAIN" in actions and "android.intent.category.LAUNCHER" in cats:
        main_activity.remove(intent)
        launcher_count += 1
if launcher_count != 1:
    raise SystemExit(f"Expected one launcher intent on MainActivity, found {launcher_count}")
diagnostic = ET.SubElement(app, "activity", {
    A("name"): ".StartupDiagnosticActivity",
    A("exported"): "true",
    A("theme"): "@android:style/Theme.Material.Light.NoActionBar",
    A("label"): "Codex Tools 啟動診斷",
})
launcher = ET.SubElement(diagnostic, "intent-filter")
ET.SubElement(launcher, "action", {A("name"): "android.intent.action.MAIN"})
ET.SubElement(launcher, "category", {A("name"): "android.intent.category.LAUNCHER"})
for name in ("AlarmReceiver","BootReceiver"):
    e=ET.SubElement(app,"receiver",{A("name"):"."+name,
        A("exported"):"true" if name=="BootReceiver" else "false"})
    if name=="BootReceiver":
        filt=ET.SubElement(e,"intent-filter")
        for action in ("android.intent.action.BOOT_COMPLETED",
                       "android.intent.action.MY_PACKAGE_REPLACED",
                       "android.app.action.SCHEDULE_EXACT_ALARM_PERMISSION_STATE_CHANGED"):
            ET.SubElement(filt,"action",{A("name"):action})
for name in ("BackupJobService","SamplingJobService"):
    ET.SubElement(app,"service",{A("name"):"."+name,A("exported"):"false",
        A("permission"):"android.permission.BIND_JOB_SERVICE"})
tree.write(manifest,encoding="utf-8",xml_declaration=True)

gradle=project/"build.gradle.kts"
if not gradle.exists():raise SystemExit("Missing app/build.gradle.kts")
with gradle.open("a") as f:
    f.write("""
dependencies {
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    implementation("com.squareup.okhttp3:okhttp-dnsoverhttps:4.12.0")
}
""")
if arguments.release_keystore is not None:
    keystore=arguments.release_keystore.expanduser().resolve(strict=True)
    # The original keystore is verified by CI before this step.  No Gradle
    # Kotlin DSL package imports or ambiguous `java` receiver are needed.
    # The Android debug-key password is intentionally the original public default.
    signing = """
android {
    signingConfigs {
        create("legacyUpgrade") {
            keyAlias = "androiddebugkey"
            keyPassword = "android"
            storeFile = file("__LEGACY_KEYSTORE_PATH__")
            storePassword = "android"
        }
    }
    buildTypes {
        getByName("release") {
            signingConfig = signingConfigs.getByName("legacyUpgrade")
            // JNI looks up LegacyBridge by exact class and method names.
            isMinifyEnabled = false
        }
    }
}
"""
    with gradle.open("a") as f:
        f.write(signing.replace("__LEGACY_KEYSTORE_PATH__", str(keystore).replace("\\\\", "/")))

print("Staged Android background scheduler, migration and release signing.")

