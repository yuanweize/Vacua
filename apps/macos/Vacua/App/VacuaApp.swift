import SwiftUI
import VacuaClient

@main
struct VacuaApp: App {
    @State private var model = AppModel()
    
    var body: some Scene {
        WindowGroup {
            ContentView(model: model)
                .task {
                    await model.startEngine()
                }
        }
        .windowStyle(.titleBar)
        .windowToolbarStyle(.unified(showsTitle: true))
        .commands {
            SidebarCommands()
            CommandGroup(replacing: .newItem) {
                Button("Refresh") {
                    Task { await model.refreshCurrentView() }
                }
                .keyboardShortcut("r", modifiers: .command)
            }
        }
        
        Settings {
            SettingsView(model: model)
                .frame(minWidth: 500, minHeight: 400)
        }
    }
}

struct ContentView: View {
    @Bindable var model: AppModel
    
    var body: some View {
        NavigationSplitView {
            List(NavigationItem.allCases, selection: $model.selectedNavigation) { item in
                NavigationLink(value: item) {
                    Label(item.rawValue, systemImage: item.iconName)
                }
            }
            .listStyle(.sidebar)
            .navigationSplitViewColumnWidth(min: 180, ideal: 200, max: 240)
            
            // Sidebar Footer: Engine Status Indicator
            VStack(alignment: .leading, spacing: 4) {
                Divider()
                HStack(spacing: 6) {
                    Circle()
                        .fill(engineStatusColor)
                        .frame(width: 8, height: 8)
                    Text(model.engineState.statusTitle)
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 8)
            }
        } detail: {
            Group {
                switch model.selectedNavigation {
                case .overview:
                    OverviewView(model: model)
                case .storageMap:
                    StorageMapView(model: model.storageMapModel)
                case .candidates:
                    CandidatesView(model: model)
                case .duplicates:
                    DuplicatesView(model: model)
                case .applications:
                    ApplicationsView(model: model)
                case .snapshots:
                    SnapshotsView(model: model)
                case .settings:
                    SettingsView(model: model)
                }
            }
            .frame(minWidth: 600, minHeight: 450)
        }
    }
    
    private var engineStatusColor: Color {
        switch model.engineState {
        case .ready: return .green
        case .starting: return .blue
        case .unavailable: return .red
        case .incompatible: return .orange
        }
    }
}
