#[cfg(test)]
mod tests {
    use crate::content::{CreateContentInput, ContentCreationStatus, CreateContentCompute};
    use collects_states::StateCtx;

    #[test]
    fn test_create_content_input_defaults() {
        let input = CreateContentInput::default();
        assert!(input.title.is_none());
        assert!(input.description.is_none());
        assert!(input.body.is_none());
        assert!(input.attachments.is_empty());
    }

    #[test]
    fn test_content_creation_status_defaults() {
        let status = ContentCreationStatus::default();
        assert_eq!(status, ContentCreationStatus::Idle);
    }

    #[test]
    fn test_content_creation_status_equality() {
        assert_eq!(ContentCreationStatus::Idle, ContentCreationStatus::Idle);
        assert_eq!(ContentCreationStatus::Uploading, ContentCreationStatus::Uploading);
        assert_eq!(
            ContentCreationStatus::Success(vec!["1".to_string()]),
            ContentCreationStatus::Success(vec!["1".to_string()])
        );
        assert_eq!(
            ContentCreationStatus::Error("err".to_string()),
            ContentCreationStatus::Error("err".to_string())
        );

        assert_ne!(ContentCreationStatus::Idle, ContentCreationStatus::Uploading);
    }

    #[test]
    fn test_create_content_compute_lifecycle() {
        let mut ctx = StateCtx::new();
        ctx.record_compute(CreateContentCompute::default());

        let compute = ctx.compute::<CreateContentCompute>();
        assert_eq!(compute.status, ContentCreationStatus::Idle);

        // Simulate update via updater (normally done by command)
        // Since we can't easily mock the command execution environment here without full setup,
        // we just verify the state/compute interactions.

        // This is primarily testing that the types are registered and usable in the system.
    }
}
