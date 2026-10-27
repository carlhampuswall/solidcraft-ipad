import Foundation

/// The same canonical/pending messages as Rust, extracted at build time for offline chrome.
enum NativeText {
    private static let catalogues: [String: [String: String]] = {
        guard let url = Bundle.main.url(forResource: "ipados-foundation", withExtension: "json"),
              let data = try? Data(contentsOf: url),
              let result = try? JSONDecoder().decode([String: [String: String]].self, from: data)
        else { return [:] }
        return result
    }()

    static func value(_ key: String) -> String {
        for identifier in Locale.preferredLanguages {
            let language = identifier.split(separator: "-").first.map(String.init) ?? identifier
            if let value = catalogues[identifier]?[key] ?? catalogues[language]?[key] { return value }
        }
        return catalogues["en"]?[key] ?? key
    }
}
