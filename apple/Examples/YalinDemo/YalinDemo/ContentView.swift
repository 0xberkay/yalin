import SwiftUI
import Yalin

struct ContentView: View {
  var body: some View {
    TabView {
      BenchmarkView().tabItem { Label("Benchmark", systemImage: "speedometer") }
      PlaygroundView().tabItem { Label("Playground", systemImage: "curlybraces") }
    }
  }
}

/// Times Yalın against Foundation on the catalog payloads.
struct BenchmarkView: View {
  @State private var dataset = Dataset.kb256
  @State private var result: BenchmarkResult?
  @State private var running = false
  @State private var error: String?

  var body: some View {
    NavigationStack {
      List {
        Section {
          Picker("Payload", selection: $dataset) {
            ForEach(Dataset.allCases) { Text($0.rawValue).tag($0) }
          }
          .pickerStyle(.menu)
          Button(running ? "Running…" : "Run") { run() }.disabled(running)
        }
        if let error { Section { Text(error).foregroundStyle(.red) } }
        if let result {
          Section("\(result.byteCount) bytes, \(result.recordCount) records") {
            if let ms = result.downloadMilliseconds {
              LabeledContent("Download", value: String(format: "%.0f ms", ms))
            }
            let best = result.timings.map(\.milliseconds).min() ?? 1
            ForEach(result.timings) { t in
              HStack {
                Text(t.name)
                Spacer()
                Text(String(format: "%.3f ms", t.milliseconds)).monospacedDigit()
                Text(String(format: "%.1f×", t.milliseconds / best))
                  .foregroundStyle(.secondary).monospacedDigit().frame(width: 52, alignment: .trailing)
              }
            }
          }
          Section("Parsed values") {
            ForEach(result.summary, id: \.self) { Text($0) }
          }
        }
      }
      .navigationTitle("Yalın")
    }
  }

  private func run() {
    running = true
    error = nil
    let selected = dataset
    Task {
      let outcome: Result<BenchmarkResult, Error>
      do {
        let downloaded = selected == .cars ? try await Benchmark.downloadCars() : nil
        outcome = await Task.detached(priority: .userInitiated) {
          Result { try Benchmark.run(selected, downloaded: downloaded) }
        }.value
      } catch {
        outcome = .failure(error)
      }
      await MainActor.run {
        switch outcome {
        case .success(let r): result = r
        case .failure(let e): error = e.localizedDescription
        }
        running = false
      }
    }
  }
}

/// Type JSON and a path, see what Yalın returns or how it fails.
struct PlaygroundView: View {
  @State private var json = #"{"user":{"name":"Ada"},"items":[{"price":12.5},{"price":3}]}"#
  @State private var path = "items[0].price"

  var body: some View {
    NavigationStack {
      Form {
        Section("JSON") {
          TextEditor(text: $json).font(.system(.body, design: .monospaced)).frame(minHeight: 120)
            .autocorrectionDisabled().textInputAutocapitalization(.never)
        }
        Section("Path") {
          TextField("user.name", text: $path).autocorrectionDisabled().textInputAutocapitalization(.never)
        }
        Section("project") { Text(projected()).font(.system(.body, design: .monospaced)) }
        Section("parseTree") { Text(tree()).font(.system(.footnote, design: .monospaced)) }
      }
      .navigationTitle("Playground")
    }
  }

  private func projected() -> String {
    do {
      let r = try Yalin.project(json, paths: [path])
      switch try r.kind(at: 0) {
      case .missing: return "missing"
      case .null: return "null"
      default:
        if let s = try? r.string(at: 0) { return "\"\(s)\"" }
        if let d = try? r.double(at: 0) { return "\(d)" }
        return "\(try r.kind(at: 0))"
      }
    } catch { return "error: \(error)" }
  }

  private func tree() -> String {
    do { return outline(try Yalin.parseTree(json)) } catch { return "error: \(error)" }
  }

  private func outline(_ value: YalinValue, indent: Int = 0) -> String {
    let pad = String(repeating: "  ", count: indent)
    switch value {
    case .object(let o):
      return o.map { "\(pad)\($0.key):\n\(outline($0.value, indent: indent + 1))" }.joined(separator: "\n")
    case .array(let items):
      return items.enumerated().map { "\(pad)[\($0.offset)]\n\(outline($0.element, indent: indent + 1))" }
        .joined(separator: "\n")
    case .string(let s): return "\(pad)\"\(s)\""
    case .int(let v): return "\(pad)\(v)"
    case .double(let v): return "\(pad)\(v)"
    case .bigNumber(let t): return "\(pad)\(t)"
    case .bool(let b): return "\(pad)\(b)"
    case .null: return "\(pad)null"
    }
  }
}
