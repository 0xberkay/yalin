# JNI binds by method name: keep YalinNative's native methods unrenamed.
-keepclasseswithmembernames class dev.yalin.YalinNative {
    native <methods>;
}
