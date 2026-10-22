# Android Rust calls these static methods by exact JNI names and signatures.
# They have no visible Java callers, so a release shrinker could strip them.
-keep class com.christorng.androidcodextools.LegacyBridge {
    public static java.lang.String *(...);
}
