use crate::scrapers::grid::{
    bbox_dimensions_km, count_initial_cells, recommended_step_for_cap, BoundingBox,
    MAX_GRID_STEP_KM, MIN_GRID_STEP_KM,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GridSafetyPolicy {
    pub max_cells_per_job: usize,
    pub max_cells_per_chunk: usize,
    pub min_step_km: f64,
    pub max_step_km: f64,
}

impl Default for GridSafetyPolicy {
    fn default() -> Self {
        Self {
            max_cells_per_job: 10_000,
            max_cells_per_chunk: 2_500,
            min_step_km: MIN_GRID_STEP_KM,
            max_step_km: MAX_GRID_STEP_KM,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GridPlan {
    pub bbox: [f64; 4],
    pub area_km2: f64,
    pub width_km: f64,
    pub height_km: f64,
    pub requested_step_km: f64,
    pub effective_step_km: f64,
    pub estimated_cells: usize,
    pub recommended_step_km: Option<f64>,
    pub exceeds_limit: bool,
    pub estimated_chunks: usize,
}

pub struct GridPlanner;

impl GridPlanner {
    pub fn plan(
        bbox: [f64; 4],
        requested_step_km: f64,
        policy: Option<GridSafetyPolicy>,
    ) -> Result<GridPlan, String> {
        let policy = policy.unwrap_or_default();
        let [lat_min, lat_max, lng_min, lng_max] = bbox;

        if !lat_min.is_finite()
            || !lat_max.is_finite()
            || !lng_min.is_finite()
            || !lng_max.is_finite()
            || !requested_step_km.is_finite()
        {
            return Err("Coordinates and step must be finite numbers".to_string());
        }
        if lat_min < -90.0 || lat_max > 90.0 || lng_min < -180.0 || lng_max > 180.0 {
            return Err(
                "Latitude or longitude is outside the supported range (-90..90, -180..180)"
                    .to_string(),
            );
        }
        if lat_min >= lat_max || lng_min >= lng_max {
            return Err("Bounding-box minimums must be less than maximums".to_string());
        }
        if requested_step_km <= 0.0 {
            return Err("Step size must be greater than zero".to_string());
        }

        let grid_bbox = BoundingBox {
            lat_min,
            lat_max,
            lng_min,
            lng_max,
        };
        let (width_km, height_km) = bbox_dimensions_km(grid_bbox);
        let area_km2 = width_km * height_km;

        let effective_step_km = requested_step_km
            .max(policy.min_step_km)
            .min(policy.max_step_km);

        let estimated_cells = count_initial_cells(grid_bbox, effective_step_km);
        let exceeds_limit = estimated_cells > policy.max_cells_per_job;

        let recommended_step_km = if exceeds_limit {
            let mut step =
                recommended_step_for_cap(grid_bbox, effective_step_km, policy.max_cells_per_job);
            step = (step * 100.0).ceil() / 100.0;
            while count_initial_cells(grid_bbox, step) > policy.max_cells_per_job {
                step += 0.01;
            }
            Some(step)
        } else {
            None
        };

        let estimated_chunks = ((estimated_cells as f64) / (policy.max_cells_per_chunk as f64))
            .ceil()
            .max(1.0) as usize;

        Ok(GridPlan {
            bbox,
            area_km2: (area_km2 * 100.0).round() / 100.0,
            width_km: (width_km * 100.0).round() / 100.0,
            height_km: (height_km * 100.0).round() / 100.0,
            requested_step_km,
            effective_step_km,
            estimated_cells,
            recommended_step_km,
            exceeds_limit,
            estimated_chunks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_planner_calculates_dimensions_and_adaptive_step() {
        // District 1 HCM (~7.7 km2) with 1.0 km step
        let plan = GridPlanner::plan([10.754, 10.798, 106.680, 106.714], 1.0, None).unwrap();
        assert!(plan.area_km2 > 0.0);
        assert!(plan.estimated_cells < 100);
        assert!(!plan.exceeds_limit);
        assert!(plan.recommended_step_km.is_none());

        // Huge bounding box with small 0.5 km step -> exceeds 10,000 limit
        let huge_plan = GridPlanner::plan([8.0, 23.0, 102.0, 110.0], 0.5, None).unwrap();
        assert!(huge_plan.exceeds_limit);
        assert!(huge_plan.recommended_step_km.is_some());
        let rec = huge_plan.recommended_step_km.unwrap();
        assert!(rec > 5.0);

        // Planning with recommended step should now be within limits
        let safe_plan = GridPlanner::plan([8.0, 23.0, 102.0, 110.0], rec, None).unwrap();
        assert!(!safe_plan.exceeds_limit);

        // Step below min_step_km (e.g. 0.01) is clamped to min_step_km (0.2)
        let min_clamped =
            GridPlanner::plan([10.754, 10.798, 106.680, 106.714], 0.01, None).unwrap();
        assert_eq!(min_clamped.requested_step_km, 0.01);
        assert_eq!(min_clamped.effective_step_km, 0.2);

        // Step above max_step_km (e.g. 150.0) is clamped to max_step_km (50.0)
        let max_clamped = GridPlanner::plan([8.0, 23.0, 102.0, 110.0], 150.0, None).unwrap();
        assert_eq!(max_clamped.requested_step_km, 150.0);
        assert_eq!(max_clamped.effective_step_km, 50.0);
        assert_eq!(max_clamped.estimated_cells, 598); // real grid at 50 km (B8), not at 150 km
    }

    #[test]
    fn planner_estimated_cells_matches_generate_initial_cells() {
        use crate::scrapers::grid::{generate_initial_cells, BoundingBox, GridLimits};

        let cases: [([f64; 4], f64); 6] = [
            ([10.754, 10.798, 106.680, 106.714], 1.0),
            ([10.0, 10.9, 106.0, 107.0], 2.0),
            ([60.0, 60.8, 10.0, 12.0], 3.0),
            ([-34.0, -33.2, 150.5, 151.5], 1.5),
            ([-0.4, 0.4, 100.0, 101.0], 2.5),
            ([71.0, 71.4, 20.0, 30.0], 5.0),
        ];
        for (bbox, step) in cases {
            let plan = GridPlanner::plan(bbox, step, None).unwrap();
            let cells = generate_initial_cells(
                BoundingBox {
                    lat_min: bbox[0],
                    lat_max: bbox[1],
                    lng_min: bbox[2],
                    lng_max: bbox[3],
                },
                step,
                GridLimits::default(),
            )
            .unwrap();
            assert_eq!(
                plan.estimated_cells,
                cells.len(),
                "planner and real grid disagree for bbox {bbox:?} step {step}"
            );
        }
    }

    #[test]
    fn test_grid_planner_rejects_invalid_inputs() {
        assert!(GridPlanner::plan([10.0, 9.0, 106.0, 107.0], 1.0, None).is_err());
        assert!(GridPlanner::plan([10.0, 11.0, 107.0, 106.0], 1.0, None).is_err());
        assert!(GridPlanner::plan([10.0, 11.0, 106.0, 107.0], 0.0, None).is_err());
        assert!(GridPlanner::plan([10.0, 11.0, 106.0, 107.0], -1.0, None).is_err());
    }
}
