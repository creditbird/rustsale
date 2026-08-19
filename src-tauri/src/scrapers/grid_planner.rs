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
            min_step_km: 0.2,
            max_step_km: 50.0,
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

        let earth_radius_km = 6_371.0088;
        let height_km =
            ((lat_max - lat_min) * std::f64::consts::PI / 180.0).abs() * earth_radius_km;

        let nearest_equator_lat = if lat_min <= 0.0 && lat_max >= 0.0 {
            0.0
        } else if lat_min.abs() < lat_max.abs() {
            lat_min
        } else {
            lat_max
        };

        let width_km = ((lng_max - lng_min) * std::f64::consts::PI / 180.0).abs()
            * earth_radius_km
            * (nearest_equator_lat * std::f64::consts::PI / 180.0)
                .cos()
                .abs()
                .max(1e-9);

        let area_km2 = width_km * height_km;

        let count_at_step = |step: f64| -> usize {
            let rows = (height_km / step).ceil().max(1.0) as usize;
            let cols = (width_km / step).ceil().max(1.0) as usize;
            rows * cols
        };

        let estimated_cells = count_at_step(requested_step_km);
        let exceeds_limit = estimated_cells > policy.max_cells_per_job;

        let recommended_step_km = if exceeds_limit {
            let mut low = requested_step_km;
            let mut high = (area_km2 / (policy.max_cells_per_job as f64))
                .sqrt()
                .max(requested_step_km)
                * 2.0;
            while count_at_step(high) > policy.max_cells_per_job {
                high *= 2.0;
            }
            for _ in 0..64 {
                let mid = (low + high) / 2.0;
                if count_at_step(mid) > policy.max_cells_per_job {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            let mut ceiled_step = (high * 100.0).ceil() / 100.0;
            while count_at_step(ceiled_step) > policy.max_cells_per_job {
                ceiled_step += 0.01;
            }
            Some(ceiled_step)
        } else {
            None
        };

        let effective_step_km = requested_step_km;
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
    }

    #[test]
    fn test_grid_planner_rejects_invalid_inputs() {
        assert!(GridPlanner::plan([10.0, 9.0, 106.0, 107.0], 1.0, None).is_err());
        assert!(GridPlanner::plan([10.0, 11.0, 107.0, 106.0], 1.0, None).is_err());
        assert!(GridPlanner::plan([10.0, 11.0, 106.0, 107.0], 0.0, None).is_err());
        assert!(GridPlanner::plan([10.0, 11.0, 106.0, 107.0], -1.0, None).is_err());
    }
}
