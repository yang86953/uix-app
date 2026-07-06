use super::*;

    #[test]
    fn register_unregister_lifecycle() {
        let mut reg = AnimationRegistry::new();
        assert!(!reg.has_active());
        reg.register(3);
        assert!(reg.has_active());
        assert!(reg.is_registered(3));
        reg.unregister(3);
        assert!(!reg.has_active());
    }

    #[test]
    fn active_ids_snapshot() {
        let mut reg = AnimationRegistry::new();
        reg.register(1);
        reg.register(5);
        let mut ids = reg.active_ids();
        ids.sort_unstable();
        assert_eq!(ids, vec![1, 5]);
    }
