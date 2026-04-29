# AGENTS.md — systemd-manager-tui

## Project
A TUI for managing systemd services on Linux. Rust edition 2024, MIT license.

## Build & Run
```sh
cargo build --release
cargo run
```

No test infrastructure exists yet. No fmt/lint step configured.

## Architecture
Clean architecture with four layers:

```
domain/          — Service, ServiceState, ServiceRepository trait
usecases/        — ServicesManager (wraps a ServiceRepository)
infrastructure/  — SystemdServiceAdapter (zbus/D-Bus impl of ServiceRepository), notifier
terminal/        — ratatui/crossterm App, event loop, UI components (list, filter, details, log)
```

## Key Patterns

### State sharing via `Rc<RefCell<>>`
`ServicesManager` is instantiated in `main.rs`, wrapped in `Rc<RefCell<>>`, and passed to all terminal components. The `terminal/app.rs` borrows the manager to drive operations.

### Event loop via `mpsc`
Components send `AppEvent` variants through `mpsc::Sender<AppEvent>`. The main event loop in `terminal/app.rs` handles them with `Actions` enums (e.g., `ReloadServices`, `Quit`, `StartService(String)`, `OpenServiceDetails(String)`).

### Trait-based repository
`ServiceRepository` trait (in `domain/service_repository.rs`) defines the data-access interface. The D-Bus adapter in `infrastructure/systemd_service_adapter.rs` implements it. This keeps domain logic testable and decoupled from systemd.

## Key Dependencies
- **ratatui 0.29** — TUI rendering
- **crossterm 0.28.1** — terminal backend
- **zbus 5.5.0** — D-Bus communication with systemd
- **rayon 1.11.0** — parallel iteration over service lists
- **chrono 0.4** — timestamps

## Release
`Cargo.toml` has a release profile with LTO, codegen-units=1, strip=true, opt-level=3. Packages for deb, rpm, AUR, Nix flake, and crates.io.

## File Naming & Structure
- Lowercase with underscores: `domain/service_repository.rs`, `terminal/components/filter.rs`
- `use super::...` for sibling module imports within a layer
- Explicit `mod/use` declarations in `main.rs`