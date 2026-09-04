import AppKit
import CoreText
import Darwin
import Security
import SwiftUI

@_silgen_name("vinny_permission_bits") private func permissionBits() -> Int32
@_silgen_name("vinny_request_permission") private func requestPermission(_ permission: Int32) -> Int32
@_silgen_name("vinny_display_ids") private func displayIDs(
    _ buffer: UnsafeMutablePointer<UInt32>?,
    _ capacity: UInt
) -> UInt
@_silgen_name("vinny_server_status") private func serverStatus(_ id: UInt64) -> Int32
@_silgen_name("vinny_start_server") private func startServer(
    _ id: UInt64,
    _ configuration: UnsafePointer<CChar>
) -> Bool
@_silgen_name("vinny_stop_server") private func stopServer(_ id: UInt64)
@_silgen_name("vinny_broadcast_clipboard") private func broadcastClipboard(
    _ bytes: UnsafePointer<UInt8>?,
    _ length: Int
)
@_silgen_name("vinny_stop_all_servers") private func stopAllServers()

private let paper = Color(red: 244 / 255, green: 242 / 255, blue: 236 / 255)
private let ink = Color(red: 23 / 255, green: 23 / 255, blue: 25 / 255)
private let blue = Color(red: 62 / 255, green: 159 / 255, blue: 255 / 255)
private let sun = Color(red: 243 / 255, green: 200 / 255, blue: 92 / 255)
private let green = Color(red: 100 / 255, green: 213 / 255, blue: 138 / 255)
private var clipboardChangeCount = NSPasteboard.general.changeCount

@_cdecl("vinny_set_clipboard")
public func vinnySetClipboard(_ bytes: UnsafePointer<UInt8>?, _ length: Int) {
    guard let bytes else { return }
    let text = String(decoding: UnsafeBufferPointer(start: bytes, count: length), as: UTF8.self)
    DispatchQueue.main.async {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(text, forType: .string)
        clipboardChangeCount = pasteboard.changeCount
    }
}

private let passwordService = "run.lil.vinny.vnc-password"

private func storedPassword(for id: UInt64) -> String {
    let query: [String: Any] = [
        kSecClass as String: kSecClassGenericPassword,
        kSecAttrService as String: passwordService,
        kSecAttrAccount as String: String(id),
        kSecReturnData as String: true,
        kSecMatchLimit as String: kSecMatchLimitOne,
    ]
    var result: CFTypeRef?
    guard SecItemCopyMatching(query as CFDictionary, &result) == errSecSuccess,
          let data = result as? Data else { return "" }
    return String(data: data, encoding: .utf8) ?? ""
}

private func storePassword(_ password: String, for id: UInt64) {
    let query: [String: Any] = [
        kSecClass as String: kSecClassGenericPassword,
        kSecAttrService as String: passwordService,
        kSecAttrAccount as String: String(id),
    ]
    if password.isEmpty {
        SecItemDelete(query as CFDictionary)
        return
    }
    let attributes: [String: Any] = [kSecValueData as String: Data(password.utf8)]
    if SecItemUpdate(query as CFDictionary, attributes as CFDictionary) == errSecItemNotFound {
        var item = query
        item[kSecValueData as String] = Data(password.utf8)
        SecItemAdd(item as CFDictionary, nil)
    }
}

private let portFormatter: NumberFormatter = {
    let formatter = NumberFormatter()
    formatter.numberStyle = .none
    formatter.usesGroupingSeparator = false
    return formatter
}()

private func parsedIPAddress(_ address: String) -> Data? {
    var ipv4 = in_addr()
    if address.withCString({ inet_pton(AF_INET, $0, &ipv4) }) == 1 {
        return Data(bytes: &ipv4, count: MemoryLayout.size(ofValue: ipv4))
    }
    var ipv6 = in6_addr()
    if address.withCString({ inet_pton(AF_INET6, $0, &ipv6) }) == 1 {
        return Data(bytes: &ipv6, count: MemoryLayout.size(ofValue: ipv6))
    }
    return nil
}

private struct DisplayOption: Identifiable, Equatable {
    let id: Int
    let name: String

    var label: String { "Display \(id + 1) (\(name))" }
}

private func connectedDisplays(captureOrder: Bool) -> [DisplayOption] {
    let screensByID: [UInt32: String] = Dictionary(uniqueKeysWithValues: NSScreen.screens.compactMap { screen -> (UInt32, String)? in
        guard let number = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber else {
            return nil
        }
        return (number.uint32Value, screen.localizedName)
    })

    if captureOrder {
        let count = Int(displayIDs(nil, 0))
        if count > 0 {
            var ids = [UInt32](repeating: 0, count: count)
            let available = ids.withUnsafeMutableBufferPointer {
                Int(displayIDs($0.baseAddress, UInt($0.count)))
            }
            return ids.prefix(min(count, available)).enumerated().map { index, id in
                DisplayOption(id: index, name: screensByID[id] ?? "Unknown display")
            }
        }
    }

    return NSScreen.screens.enumerated().map {
        DisplayOption(id: $0.offset, name: $0.element.localizedName)
    }
}

private enum SharingPolicy: String, Codable, CaseIterable {
    case followClient
    case alwaysShared
    case singleClient

    var label: String {
        switch self {
        case .followClient: "Viewer decides"
        case .alwaysShared: "Allow multiple viewers"
        case .singleClient: "One viewer at a time"
        }
    }

    var explanation: String {
        switch self {
        case .followClient: "A viewer can share the session or request exclusive access."
        case .alwaysShared: "New viewers join without disconnecting anyone."
        case .singleClient: "New connections are rejected while a viewer is connected."
        }
    }
}

private struct ServerConfiguration: Codable, Identifiable, Equatable {
    var id: UInt64
    var display: Int
    var port: Int
    var maxWidth: Int
    var fps: Int
    var address: String
    var enabled: Bool
    var sharingPolicy: SharingPolicy
    var viewOnly: Bool
    var secure: Bool

    init(
        id: UInt64,
        display: Int,
        port: Int,
        maxWidth: Int,
        fps: Int,
        address: String,
        enabled: Bool,
        sharingPolicy: SharingPolicy = .followClient,
        viewOnly: Bool = false,
        secure: Bool = false
    ) {
        self.id = id
        self.display = display
        self.port = port
        self.maxWidth = maxWidth
        self.fps = fps
        self.address = address
        self.enabled = enabled
        self.sharingPolicy = sharingPolicy
        self.viewOnly = viewOnly
        self.secure = secure
    }

    private enum CodingKeys: String, CodingKey {
        case id, display, port, maxWidth, fps, address, enabled, sharingPolicy, viewOnly, secure
    }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        id = try values.decode(UInt64.self, forKey: .id)
        display = try values.decode(Int.self, forKey: .display)
        port = try values.decode(Int.self, forKey: .port)
        maxWidth = try values.decode(Int.self, forKey: .maxWidth)
        fps = try values.decode(Int.self, forKey: .fps)
        address = try values.decode(String.self, forKey: .address)
        enabled = try values.decode(Bool.self, forKey: .enabled)
        sharingPolicy = try values.decodeIfPresent(SharingPolicy.self, forKey: .sharingPolicy) ?? .followClient
        viewOnly = try values.decodeIfPresent(Bool.self, forKey: .viewOnly) ?? false
        secure = try values.decodeIfPresent(Bool.self, forKey: .secure) ?? false
    }

    static let primary = ServerConfiguration(
        id: 1,
        display: 0,
        port: 5900,
        maxWidth: 1920,
        fps: 20,
        address: "127.0.0.1",
        enabled: true
    )
}

private struct RuntimeServerConfiguration: Encodable {
    let address: String
    let port: Int
    let display: Int
    let maxWidth: Int
    let fps: Int
    let sharingPolicy: SharingPolicy
    let viewOnly: Bool
    let password: String?

    init(configuration: ServerConfiguration, password: String?) {
        address = configuration.address
        port = configuration.port
        display = configuration.display
        maxWidth = configuration.maxWidth
        fps = configuration.fps
        sharingPolicy = configuration.sharingPolicy
        viewOnly = configuration.viewOnly
        self.password = password
    }
}

private final class VinnyModel: ObservableObject {
    @Published var screenRecording = false
    @Published var accessibility = false
    @Published var displays: [DisplayOption]
    @Published var servers: [ServerConfiguration]
    @Published var statuses: [UInt64: Int32] = [:]
    @Published var errors: [UInt64: String] = [:]
    @Published var passwords: [UInt64: String]

    private let defaultsKey = "serverConfigurations"
    private var attempted = Set<UInt64>()
    private var timer: Timer?
    private var screenObserver: NSObjectProtocol?

    init() {
        displays = connectedDisplays(captureOrder: false)
        passwords = [:]
        if let data = UserDefaults.standard.data(forKey: defaultsKey),
           let saved = try? JSONDecoder().decode([ServerConfiguration].self, from: data),
           !saved.isEmpty {
            servers = saved
        } else {
            servers = [.primary]
        }
        passwords = Dictionary(uniqueKeysWithValues: servers.map { ($0.id, storedPassword(for: $0.id)) })
        refresh()
        timer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            self?.refresh()
        }
        screenObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.didChangeScreenParametersNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.reloadDisplays()
        }
    }

    deinit {
        timer?.invalidate()
        if let screenObserver { NotificationCenter.default.removeObserver(screenObserver) }
    }

    var ready: Bool { screenRecording && accessibility }

    func refresh() {
        let hadScreenRecording = screenRecording
        let wasReady = ready
        let bits = permissionBits()
        screenRecording = bits & 1 != 0
        accessibility = bits & 2 != 0
        if screenRecording && !hadScreenRecording {
            reloadDisplays()
        }
        if wasReady && !ready {
            for server in servers {
                stopServer(server.id)
                statuses[server.id] = 0
                attempted.remove(server.id)
            }
        }
        pollClipboard()
        for server in servers {
            statuses[server.id] = serverStatus(server.id)
            if ready, server.enabled, statuses[server.id] == 0, !attempted.contains(server.id) {
                apply(server)
            }
        }
    }

    func allowScreenRecording() {
        _ = requestPermission(1)
        refresh()
    }

    func allowAccessibility() {
        _ = requestPermission(2)
        refresh()
    }

    func addServer() {
        let usedPorts = Set(servers.map(\.port))
        var port = 5900
        while usedPorts.contains(port) { port += 1 }
        let display = min(servers.count, max(displays.count - 1, 0))
        let server = ServerConfiguration(
            id: UInt64.random(in: 2...UInt64.max),
            display: display,
            port: port,
            maxWidth: 1920,
            fps: 20,
            address: "127.0.0.1",
            enabled: false
        )
        servers.append(server)
        passwords[server.id] = ""
        save()
    }

    func apply(_ configuration: ServerConfiguration) {
        save()
        stopServer(configuration.id)
        statuses[configuration.id] = 0
        errors[configuration.id] = nil
        attempted.insert(configuration.id)

        guard configuration.enabled, ready else { return }
        guard validate(configuration) else { return }
        let password = passwords[configuration.id] ?? ""
        if configuration.secure && password.isEmpty {
            errors[configuration.id] = "Enter a password for encrypted connections."
            return
        }
        storePassword(configuration.secure ? password : "", for: configuration.id)
        let runtime = RuntimeServerConfiguration(
            configuration: configuration,
            password: configuration.secure ? password : nil
        )

        guard let data = try? JSONEncoder().encode(runtime),
              let json = String(data: data, encoding: .utf8) else {
            errors[configuration.id] = "Could not encode this server configuration."
            return
        }
        let started = json.withCString { startServer(configuration.id, $0) }
        if started {
            statuses[configuration.id] = serverStatus(configuration.id)
        } else {
            errors[configuration.id] = "Check the address and permissions, then try again."
        }
    }

    func remove(_ configuration: ServerConfiguration) {
        stopServer(configuration.id)
        servers.removeAll { $0.id == configuration.id }
        statuses[configuration.id] = nil
        errors[configuration.id] = nil
        attempted.remove(configuration.id)
        passwords[configuration.id] = nil
        storePassword("", for: configuration.id)
        if servers.isEmpty { servers = [.primary] }
        save()
    }

    private func validate(_ configuration: ServerConfiguration) -> Bool {
        if !displays.contains(where: { $0.id == configuration.display }) {
            errors[configuration.id] = "Choose a connected display."
            return false
        }
        if !(1...65535).contains(configuration.port) {
            errors[configuration.id] = "Port must be between 1 and 65535."
            return false
        }
        if !(1...60).contains(configuration.fps) {
            errors[configuration.id] = "Frame rate must be between 1 and 60."
            return false
        }
        if !(320...7680).contains(configuration.maxWidth) {
            errors[configuration.id] = "Maximum width must be between 320 and 7680."
            return false
        }
        let address = configuration.address.trimmingCharacters(in: .whitespacesAndNewlines)
        if address.isEmpty {
            errors[configuration.id] = "Enter an IP address assigned to this Mac."
            return false
        }
        guard let parsedAddress = parsedIPAddress(configuration.address) else {
            errors[configuration.id] = "Enter a valid IPv4 or IPv6 address."
            return false
        }
        if let currentIndex = servers.firstIndex(where: { $0.id == configuration.id }),
           let duplicateIndex = servers[..<currentIndex].firstIndex(where: {
               $0.enabled && $0.port == configuration.port && parsedIPAddress($0.address) == parsedAddress
           }) {
            errors[configuration.id] = "Server \(duplicateIndex + 1) already uses this address and port."
            return false
        }
        return true
    }

    private func pollClipboard() {
        let pasteboard = NSPasteboard.general
        guard pasteboard.changeCount != clipboardChangeCount else { return }
        clipboardChangeCount = pasteboard.changeCount
        guard let text = pasteboard.string(forType: .string) else { return }
        let bytes = Array(text.utf8)
        bytes.withUnsafeBufferPointer { broadcastClipboard($0.baseAddress, $0.count) }
    }

    private func reloadDisplays() {
        displays = connectedDisplays(captureOrder: screenRecording)
    }

    private func save() {
        if let data = try? JSONEncoder().encode(servers) {
            UserDefaults.standard.set(data, forKey: defaultsKey)
        }
    }
}

private struct PermissionCard: View {
    let title: String
    let detail: String
    let granted: Bool
    let action: () -> Void

    var body: some View {
        HStack(spacing: 14) {
            Circle()
                .fill(granted ? green : sun)
                .frame(width: 11, height: 11)
                .overlay {
                    if granted {
                        Image(systemName: "checkmark")
                            .font(.system(size: 7, weight: .bold))
                            .foregroundColor(ink)
                    }
                }
            VStack(alignment: .leading, spacing: 3) {
                Text(title)
                    .font(.custom("Maple Mono", size: 13))
                    .foregroundColor(.white)
                Text(detail)
                    .font(.custom("Maple Mono", size: 12))
                    .foregroundColor(Color.white.opacity(0.58))
            }
            Spacer()
            Button(granted ? "Allowed" : "Allow", action: action)
                .buttonStyle(VinnyButtonStyle(enabled: !granted))
                .disabled(granted)
        }
        .padding(17)
        .background(ink)
        .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
        .shadow(color: sun, radius: 0, x: 6, y: 6)
        .padding(.trailing, 6)
        .padding(.bottom, 6)
    }
}

private struct VinnyButtonStyle: ButtonStyle {
    let enabled: Bool

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.custom("Maple Mono", size: 12))
            .foregroundColor(enabled ? .white : Color.white.opacity(0.55))
            .padding(.horizontal, 13)
            .frame(height: 32)
            .background(enabled ? blue : Color.white.opacity(0.12))
            .clipShape(RoundedRectangle(cornerRadius: 9, style: .continuous))
            .scaleEffect(configuration.isPressed ? 0.97 : 1)
    }
}

private struct VinnySecondaryButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.custom("Maple Mono", size: 12))
            .foregroundColor(ink.opacity(0.72))
            .padding(.horizontal, 13)
            .frame(height: 32)
            .background(ink.opacity(configuration.isPressed ? 0.12 : 0.07))
            .clipShape(RoundedRectangle(cornerRadius: 9, style: .continuous))
            .scaleEffect(configuration.isPressed ? 0.97 : 1)
    }
}

private struct ServerCard: View {
    @Binding var configuration: ServerConfiguration
    @Binding var password: String
    let number: Int
    let displays: [DisplayOption]
    let status: Int32
    let error: String?
    let apply: () -> Void
    let remove: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack {
                Circle().fill(statusColor).frame(width: 9, height: 9)
                Text("Server \(number)")
                    .font(.custom("Maple Mono", size: 14))
                    .foregroundColor(ink)
                Text(statusText)
                    .font(.custom("Maple Mono", size: 11))
                    .foregroundColor(ink.opacity(0.55))
                Spacer()
                Toggle("Enabled", isOn: $configuration.enabled)
                    .toggleStyle(.switch)
                    .labelsHidden()
            }

            VStack(spacing: 14) {
                settingRow("Display") {
                    Menu {
                        ForEach(displays) { display in
                            Button(display.label) {
                                configuration.display = display.id
                            }
                        }
                    } label: {
                        Text(selectedDisplayLabel)
                            .lineLimit(1)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .contentShape(Rectangle())
                    }
                }
                settingRow("Maximum width") {
                    TextField("1920", value: $configuration.maxWidth, format: .number)
                        .textFieldStyle(.roundedBorder)
                }
                settingRow("Frame rate") {
                    Stepper("\(configuration.fps) FPS", value: $configuration.fps, in: 1...60)
                }
                settingRow("Viewers") {
                    VStack(alignment: .leading, spacing: 5) {
                        Menu {
                            ForEach(SharingPolicy.allCases, id: \.self) { policy in
                                Button(policy.label) { configuration.sharingPolicy = policy }
                            }
                        } label: {
                            Text(configuration.sharingPolicy.label)
                                .frame(maxWidth: .infinity, alignment: .leading)
                        }
                        Text(configuration.sharingPolicy.explanation)
                            .font(.custom("Maple Mono", size: 11))
                            .foregroundColor(ink.opacity(0.58))
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                settingRow("Remote control") {
                    VStack(alignment: .leading, spacing: 5) {
                        Toggle(
                            "Allow keyboard and mouse",
                            isOn: Binding(
                                get: { !configuration.viewOnly },
                                set: { configuration.viewOnly = !$0 }
                            )
                        )
                        Text("Off blocks keyboard, mouse, and incoming clipboard changes.")
                            .font(.custom("Maple Mono", size: 11))
                            .foregroundColor(ink.opacity(0.58))
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                settingRow("Security") {
                    Toggle("Encrypted + password", isOn: $configuration.secure)
                }
                if configuration.secure {
                    settingRow("Password") {
                        SecureField("Required", text: $password)
                            .textFieldStyle(.roundedBorder)
                    }
                }
                settingRow("Listen on") {
                    HStack(spacing: 8) {
                        TextField("127.0.0.1", text: $configuration.address)
                            .textFieldStyle(.roundedBorder)
                            .accessibilityLabel("Listen address")
                        Text(":")
                            .foregroundColor(ink.opacity(0.5))
                        TextField("5900", value: $configuration.port, formatter: portFormatter)
                            .textFieldStyle(.roundedBorder)
                            .frame(width: 82)
                            .accessibilityLabel("Port")
                    }
                }
                if !configuration.secure
                    && configuration.address != "127.0.0.1"
                    && configuration.address != "::1" {
                    Text("This listener is unauthenticated and plaintext. Use only trusted networks or a secure tunnel.")
                        .font(.custom("Maple Mono", size: 11))
                        .foregroundColor(ink.opacity(0.62))
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.leading, 128)
                }
            }

            if let error {
                Text(error)
                    .font(.custom("Maple Mono", size: 11))
                    .foregroundColor(.red)
            }

            HStack {
                Button("Remove", action: remove)
                    .buttonStyle(VinnySecondaryButtonStyle())
                Spacer()
                Button(configuration.enabled ? "Apply & restart" : "Stop server", action: apply)
                    .buttonStyle(VinnyButtonStyle(enabled: true))
            }
        }
        .padding(22)
        .background(Color.white.opacity(0.72))
        .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .stroke(ink.opacity(0.1), lineWidth: 1)
        }
    }

    private func settingRow<Content: View>(
        _ label: String,
        @ViewBuilder content: () -> Content
    ) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 20) {
            Text(label)
                .font(.custom("Maple Mono", size: 12).weight(.medium))
                .foregroundColor(ink.opacity(0.7))
                .frame(width: 120, alignment: .leading)
            content()
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private var selectedDisplayLabel: String {
        displays.first(where: { $0.id == configuration.display })?.label
            ?? "Display \(configuration.display + 1) (disconnected)"
    }

    private var statusColor: Color {
        if !configuration.enabled { return ink.opacity(0.2) }
        if status == 1 { return green }
        if status == 2 || error != nil { return .red }
        return sun
    }

    private var statusText: String {
        if !configuration.enabled { return "stopped" }
        if status == 1 { return "listening" }
        if status == 2 || error != nil { return "needs attention" }
        return "waiting"
    }
}

private struct ContentView: View {
    @StateObject private var model = VinnyModel()

    var body: some View {
        ScrollView {
            VStack(spacing: 0) {
                header.padding(.bottom, 22)

                VStack(spacing: 14) {
                    PermissionCard(
                        title: "Screen recording",
                        detail: "Lets Vinny show this Mac over VNC.",
                        granted: model.screenRecording,
                        action: model.allowScreenRecording
                    )
                    PermissionCard(
                        title: "Accessibility",
                        detail: "Lets remote keyboard and pointer input work.",
                        granted: model.accessibility,
                        action: model.allowAccessibility
                    )
                }

                HStack {
                    Text("Servers")
                        .font(.custom("Kinder Child Kawaii Bubble", size: 28))
                        .foregroundColor(ink)
                    Spacer()
                    Button("Add server", action: model.addServer)
                        .buttonStyle(VinnyButtonStyle(enabled: true))
                }
                .padding(.top, 30)
                .padding(.bottom, 12)

                VStack(spacing: 14) {
                    ForEach(Array(model.servers.enumerated()), id: \.element.id) { index, server in
                        if let binding = binding(for: server.id) {
                            ServerCard(
                                configuration: binding,
                                password: passwordBinding(for: server.id),
                                number: index + 1,
                                displays: model.displays,
                                status: model.statuses[server.id] ?? 0,
                                error: model.errors[server.id],
                                apply: { model.apply(binding.wrappedValue) },
                                remove: { model.remove(binding.wrappedValue) }
                            )
                        }
                    }
                }
            }
            .padding(28)
        }
        .frame(width: 680, height: 820)
        .font(.custom("Maple Mono", size: 12))
        .background(paper)
        .preferredColorScheme(.light)
    }

    private var header: some View {
        HStack(alignment: .center, spacing: 22) {
            VStack(alignment: .leading, spacing: 5) {
                Text("vinny")
                    .font(.custom("Kinder Child Kawaii Bubble", size: 60))
                    .foregroundColor(ink)
                Text("A tiny VNC server for your Mac")
                    .font(.custom("Kinder Child Kawaii Bubble", size: 18))
                    .foregroundColor(ink.opacity(0.65))
            }
            Spacer()
            if let image = NSImage(named: "OnboardingMascot") {
                Image(nsImage: image)
                    .resizable()
                    .scaledToFit()
                    .frame(width: 104, height: 104)
            }
        }
    }

    private func passwordBinding(for id: UInt64) -> Binding<String> {
        Binding(
            get: { model.passwords[id] ?? "" },
            set: { model.passwords[id] = $0 }
        )
    }

    private func binding(for id: UInt64) -> Binding<ServerConfiguration>? {
        guard let index = model.servers.firstIndex(where: { $0.id == id }) else { return nil }
        return $model.servers[index]
    }
}

private func vinnyMenuBarIcon() -> NSImage {
    let image = NSImage(size: NSSize(width: 18, height: 18), flipped: false) { _ in
        NSColor.black.setStroke()
        NSColor.black.setFill()

        let body = NSBezierPath(roundedRect: NSRect(x: 2, y: 2.5, width: 14, height: 11), xRadius: 3.5, yRadius: 3.5)
        body.lineWidth = 1.35
        body.stroke()

        let screen = NSBezierPath(roundedRect: NSRect(x: 4.2, y: 5, width: 9.6, height: 6.2), xRadius: 2, yRadius: 2)
        screen.lineWidth = 1.1
        screen.stroke()

        NSBezierPath(ovalIn: NSRect(x: 6.1, y: 7.2, width: 1.25, height: 1.8)).fill()
        NSBezierPath(ovalIn: NSRect(x: 10.65, y: 7.2, width: 1.25, height: 1.8)).fill()

        let smile = NSBezierPath()
        smile.move(to: NSPoint(x: 7.7, y: 6.65))
        smile.curve(
            to: NSPoint(x: 10.3, y: 6.65),
            controlPoint1: NSPoint(x: 8.35, y: 5.85),
            controlPoint2: NSPoint(x: 9.65, y: 5.85)
        )
        smile.lineWidth = 1
        smile.lineCapStyle = .round
        smile.stroke()

        let antenna = NSBezierPath()
        antenna.move(to: NSPoint(x: 9, y: 13.4))
        antenna.curve(
            to: NSPoint(x: 10.7, y: 16),
            controlPoint1: NSPoint(x: 9, y: 14.6),
            controlPoint2: NSPoint(x: 9.8, y: 15.7)
        )
        antenna.lineWidth = 1.25
        antenna.lineCapStyle = .round
        antenna.stroke()
        NSBezierPath(ovalIn: NSRect(x: 10.15, y: 15.45, width: 1.5, height: 1.5)).fill()

        return true
    }
    image.isTemplate = true
    return image
}

private final class VinnyAppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate {
    private var window: NSWindow?
    private var statusItem: NSStatusItem?

    func applicationDidFinishLaunching(_ notification: Notification) {
        registerFonts()

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 620, height: 720),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = "Vinny"
        window.minSize = NSSize(width: 620, height: 560)
        window.isReleasedWhenClosed = false
        window.delegate = self
        window.center()
        window.contentViewController = NSHostingController(rootView: ContentView())
        self.window = window

        let statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        statusItem.button?.image = vinnyMenuBarIcon()
        statusItem.button?.toolTip = "Vinny"
        let menu = NSMenu()
        menu.addItem(withTitle: "Show Vinny", action: #selector(showWindow), keyEquivalent: "")
        menu.addItem(.separator())
        menu.addItem(withTitle: "Quit Vinny", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        statusItem.menu = menu
        self.statusItem = statusItem

        showWindow()
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { false }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        showWindow()
        return true
    }

    func applicationWillTerminate(_ notification: Notification) {
        stopAllServers()
    }

    func windowWillClose(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
    }

    @objc private func showWindow() {
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
        window?.makeKeyAndOrderFront(nil)
    }

    private func registerFonts() {
        guard let resources = Bundle.main.resourceURL else { return }
        for name in ["kinder-child-kawaii-bubble.otf", "maple-mono-regular.ttf"] {
            let url = resources.appendingPathComponent(name) as CFURL
            CTFontManagerRegisterFontsForURL(url, .process, nil)
        }
    }
}

private var retainedDelegate: VinnyAppDelegate?

@_cdecl("vinny_run_gui")
public func vinnyRunGUI() {
    let app = NSApplication.shared
    let delegate = VinnyAppDelegate()
    retainedDelegate = delegate
    app.delegate = delegate
    app.setActivationPolicy(.accessory)
    app.run()
}
