use pool_core::models::{Project, Shot, Workflow};

#[test]
fn test_project_creation() {
    let project = Project::new("Test Project".to_string());
    assert_eq!(project.name, "Test Project");
    assert!(!project.id.is_empty());
}

#[test]
fn test_project_add_shot() {
    let mut project = Project::new("Test Project".to_string());
    let shot = Shot::new("Shot 1".to_string());
    project.add_shot(shot.clone());
    assert_eq!(project.shots.len(), 1);
    assert_eq!(project.shots[0].name, "Shot 1");
}
