//! Tests for the database layer
//!
//! These tests verify the database initialization and CRUD operations.

use pool_core::db::Database;
use pool_core::models::Project;

#[tokio::test]
async fn test_database_init() {
    let db = Database::new(":memory:").await.unwrap();
    assert!(db.is_healthy().await);
}

#[tokio::test]
async fn test_project_crud() {
    let db = Database::new(":memory:").await.unwrap();

    // Create
    let project = Project::new("Test Project".to_string());
    let id = project.id.clone();
    db.create_project(&project).await.unwrap();

    // Read
    let found = db.get_project(&id).await.unwrap();
    assert_eq!(found.name, "Test Project");

    // Update
    let mut updated = found.clone();
    updated.name = "Updated Project".to_string();
    db.update_project(&updated).await.unwrap();

    let found = db.get_project(&id).await.unwrap();
    assert_eq!(found.name, "Updated Project");

    // Delete
    db.delete_project(&id).await.unwrap();
    assert!(db.get_project(&id).await.is_err());
}

#[tokio::test]
async fn test_list_projects() {
    let db = Database::new(":memory:").await.unwrap();

    // Create multiple projects
    let project1 = Project::new("Project 1".to_string());
    let project2 = Project::new("Project 2".to_string());
    let project3 = Project::new("Project 3".to_string());

    db.create_project(&project1).await.unwrap();
    db.create_project(&project2).await.unwrap();
    db.create_project(&project3).await.unwrap();

    // List all projects
    let projects = db.list_projects().await.unwrap();
    assert_eq!(projects.len(), 3);
}

#[tokio::test]
async fn test_shot_crud() {
    use pool_core::models::{Shot, ShotStatus};

    let db = Database::new(":memory:").await.unwrap();

    // Create project first
    let project = Project::new("Test Project".to_string());
    let project_id = project.id.clone();
    db.create_project(&project).await.unwrap();

    // Create shot
    let shot = Shot::new("Test Shot".to_string()).with_project(project_id.clone());
    let shot_id = shot.id.clone();
    db.create_shot(&shot).await.unwrap();

    // Read
    let found = db.get_shot(&shot_id).await.unwrap();
    assert_eq!(found.name, "Test Shot");
    assert_eq!(found.project_id, project_id);
    assert_eq!(found.status, ShotStatus::Idle);

    // Update
    let mut updated = found.clone();
    updated.name = "Updated Shot".to_string();
    updated.status = ShotStatus::Processing;
    db.update_shot(&updated).await.unwrap();

    let found = db.get_shot(&shot_id).await.unwrap();
    assert_eq!(found.name, "Updated Shot");
    assert_eq!(found.status, ShotStatus::Processing);

    // Delete
    db.delete_shot(&shot_id).await.unwrap();
    assert!(db.get_shot(&shot_id).await.is_err());
}

#[tokio::test]
async fn test_list_shots_by_project() {
    use pool_core::models::Shot;

    let db = Database::new(":memory:").await.unwrap();

    // Create project
    let project = Project::new("Test Project".to_string());
    let project_id = project.id.clone();
    db.create_project(&project).await.unwrap();

    // Create shots
    let shot1 = Shot::new("Shot 1".to_string()).with_project(project_id.clone());
    let shot2 = Shot::new("Shot 2".to_string()).with_project(project_id.clone());

    db.create_shot(&shot1).await.unwrap();
    db.create_shot(&shot2).await.unwrap();

    // List shots
    let shots = db.list_shots_by_project(&project_id).await.unwrap();
    assert_eq!(shots.len(), 2);
}

#[tokio::test]
async fn test_workflow_crud() {
    use pool_core::models::Workflow;

    let db = Database::new(":memory:").await.unwrap();

    // Create project and shot first
    let project = Project::new("Test Project".to_string());
    let project_id = project.id.clone();
    db.create_project(&project).await.unwrap();

    use pool_core::models::Shot;
    let shot = Shot::new("Test Shot".to_string()).with_project(project_id);
    let shot_id = shot.id.clone();
    db.create_shot(&shot).await.unwrap();

    // Create workflow
    let workflow = Workflow::new("Test Workflow".to_string(), shot_id.clone());
    let workflow_id = workflow.id.clone();
    db.create_workflow(&workflow).await.unwrap();

    // Read
    let found = db.get_workflow(&workflow_id).await.unwrap();
    assert_eq!(found.name, "Test Workflow");
    assert_eq!(found.shot_id, shot_id);

    // Get by shot
    let by_shot = db.get_workflow_by_shot(&shot_id).await.unwrap();
    assert!(by_shot.is_some());
    assert_eq!(by_shot.unwrap().name, "Test Workflow");

    // Update
    let mut updated = found.clone();
    updated.name = "Updated Workflow".to_string();
    db.update_workflow(&updated).await.unwrap();

    let found = db.get_workflow(&workflow_id).await.unwrap();
    assert_eq!(found.name, "Updated Workflow");

    // Delete
    db.delete_workflow(&workflow_id).await.unwrap();
    assert!(db.get_workflow(&workflow_id).await.is_err());
}
