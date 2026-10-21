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

activity=next(project.rglob("MainActivity.kt"),None)
if activity is None:raise SystemExit("Missing generated MainActivity.kt")
original=activity.read_text()
prefix=original.split("class MainActivity",1)[0]
if "import android.os.Bundle" not in prefix:
    prefix+="\nimport android.os.Bundle\n"
activity.write_text(prefix+"""
class MainActivity : TauriActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        AndroidNotifications.ensureChannel(this)
        Scheduler.scheduleQuotaSampling(this)
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

