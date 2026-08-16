#[cfg(test)]
mod tests {
    use smithay::reexports::wayland_server::Display;
    use truss::App;

    #[test]
    fn test_app_initialization() {
        let mut display: Display<App> = Display::new().unwrap();
        let app = App::new(&mut display).unwrap();

        assert_eq!(app.state.active_workspace_id, 1);
        assert_eq!(app.state.workspaces.len(), 9);
        assert!(app.surfaces.is_empty());
        assert!(app.clients.is_empty());
    }
}
