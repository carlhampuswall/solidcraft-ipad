enum ProjectionText {
    static func text(_ key: String) -> String { NativeText.value("ui.ipados.projection." + key) }
}
