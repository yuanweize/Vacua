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
            List(selection: $model.selectedNavigation) {
                Section {
                    ForEach(NavigationItem.coreItems) { item in
                        NavigationLink(value: item) {
                            Label(item.rawValue, systemImage: item.iconName)
                        }
                    }
                }
                
                Section("Analysis") {
                    ForEach(NavigationItem.analysisItems) { item in
                        NavigationLink(value: item) {
                            Label(item.rawValue, systemImage: item.iconName)
                        }
                    }
                }
            }
            .listStyle(.sidebar)
            .navigationSplitViewColumnWidth(min: 190, ideal: 210, max: 250)
            
            // Sidebar Footer: Engine Status Indicator
            VStack(alignment: .leading, spacing: 4) {
                Divider()
                HStack(spacing: 8) {
                    Circle()
                        .fill(engineStatusColor)
                        .frame(width: 8, height: 8)
                    Text(model.engineState.statusTitle)
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                    Spacer()
                    Text(model.activeRootDisplayName)
                        .font(.caption2.monospaced())
                        .foregroundStyle(.tertiary)
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
                }
            }
            .frame(minWidth: 620, minHeight: 460)
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
