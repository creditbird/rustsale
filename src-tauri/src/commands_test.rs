#[cfg(test)]
mod tests {
    use super::super::commands;
    use super::super::db;
    use std::sync::Mutex;

    #[test]
    fn test_crm_commands() {
        let conn = db::init_db(":memory:").unwrap();
        let state = Mutex::new(conn);

        // Test get_leads_internal
        let leads = commands::get_leads_internal(&state).unwrap();
        assert_eq!(leads.len(), 2);
        assert_eq!(leads[0].name, "Sarah Harrington");
        assert_eq!(leads[0].about, Some("CTO of Acme Corp".to_string()));

        // Test get_lead_details
        let lead = commands::get_lead_details_internal(&state, 1).unwrap();
        assert_eq!(lead.name, "Sarah Harrington");

        // Test add_lead_internal
        let new_lead_id = commands::add_lead_internal(
            &state,
            "John Doe".to_string(),
            "Example Inc".to_string(),
            "New".to_string(),
            5000.0,
            "Alex Rivera".to_string(),
            "john@example.com".to_string(),
            "+11223344".to_string(),
            Some("CEO".to_string()),
        ).unwrap();
        assert!(new_lead_id > 0);

        let leads_updated = commands::get_leads_internal(&state).unwrap();
        assert_eq!(leads_updated.len(), 3);

        // Test update_lead_status_internal
        commands::update_lead_status_internal(&state, new_lead_id, "Contacted".to_string()).unwrap();
        let lead_updated = commands::get_lead_details_internal(&state, new_lead_id).unwrap();
        assert_eq!(lead_updated.status, "Contacted");

        // Test get_deals
        let deals = commands::get_deals_internal(&state).unwrap();
        assert_eq!(deals.len(), 2);

        // Test update_deal_stage_internal
        commands::update_deal_stage_internal(&state, 1, "Demo".to_string()).unwrap();
        let deals_updated = commands::get_deals_internal(&state).unwrap();
        assert_eq!(deals_updated[0].stage, "Demo");

        // Test get_activities
        let activities = commands::get_activities_internal(&state, 1).unwrap();
        assert_eq!(activities.len(), 1);
        assert_eq!(activities[0].activity_type, "Email");

        // Test add_activity_internal
        commands::add_activity_internal(&state, 1, "Call".to_string(), "Intro call".to_string(), "Alex Rivera".to_string()).unwrap();
        let activities_updated = commands::get_activities_internal(&state, 1).unwrap();
        assert_eq!(activities_updated.len(), 2);
        assert_eq!(activities_updated[1].activity_type, "Call");
    }
}
