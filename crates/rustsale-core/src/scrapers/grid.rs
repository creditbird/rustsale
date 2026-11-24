use crate::google_maps_jobs::validation::{validate_and_estimate_grid, ScrapeError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
    pub lat_min: f64,
    pub lat_max: f64,
    pub lng_min: f64,
    pub lng_max: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GridCell {
    pub bbox: BoundingBox,
    pub center_lat: f64,
    pub center_lng: f64,
    pub step_lat_deg: f64,
    pub step_lng_deg: f64,
    pub radius_m: f64,
    pub depth: u8,
    pub parent_unit_id: Option<String>,
    pub consecutive_low_yield_levels: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GridLimits {
    pub max_depth: u8,
    pub min_cell_size_m: f64,
    pub max_initial_cells: usize,
    pub max_total_cells: usize,
}

impl Default for GridLimits {
    fn default() -> Self {
        Self {
            max_depth: 6,
            min_cell_size_m: 250.0,
            max_initial_cells: 10_000,
            max_total_cells: 25_000,
        }
    }
}

/// Finest zoom `radius_to_zoom` returns; splitting a cell already at it adds no detail.
const MAX_GRID_ZOOM: u8 = 18;

pub fn radius_to_zoom(radius_m: f64) -> u8 {
    if radius_m >= 10000.0 {
        12
    } else if radius_m >= 5000.0 {
        13
    } else if radius_m >= 2500.0 {
        14
    } else if radius_m >= 1200.0 {
        15
    } else if radius_m >= 600.0 {
        16
    } else if radius_m >= 300.0 {
        17
    } else {
        18
    }
}

/// Kilometres per degree of latitude used by every grid calculation (planner, validation, cells).
pub const KM_PER_DEGREE: f64 = 111.0;
/// Smallest/largest `step_km` the backend accepts for a grid job.
pub const MIN_GRID_STEP_KM: f64 = 0.2;
pub const MAX_GRID_STEP_KM: f64 = 50.0;

/// One row of the initial grid: its latitude band and how many columns it holds.
struct GridRow {
    lat_min: f64,
    lat_max: f64,
    lng_step_deg: f64,
    columns: usize,
}

/// Rows of the initial grid. The only place that converts km to degrees and decides how
/// many cells a row has, so cell generation, counting and the planner cannot disagree.
fn initial_grid_rows(bbox: BoundingBox, step_km: f64) -> impl Iterator<Item = GridRow> {
    let lat_step_deg = step_km / KM_PER_DEGREE;
    let rows = ((bbox.lat_max - bbox.lat_min) / lat_step_deg)
        .ceil()
        .max(1.0) as usize;
    (0..rows).map_while(move |row| {
        let lat_min = bbox.lat_min + row as f64 * lat_step_deg;
        if lat_min >= bbox.lat_max {
            return None;
        }
        let lat_max = (lat_min + lat_step_deg).min(bbox.lat_max);
        let center_lat = (lat_min + lat_max) / 2.0;
        let cos_lat = center_lat.to_radians().cos().abs();
        let lng_step_deg = if cos_lat > 1e-9 {
            step_km / (KM_PER_DEGREE * cos_lat)
        } else {
            bbox.lng_max - bbox.lng_min
        };
        let mut columns = ((bbox.lng_max - bbox.lng_min) / lng_step_deg)
            .ceil()
            .max(1.0) as usize;
        // Floating point can put the last column's start on or past the edge.
        while columns > 1 && bbox.lng_min + (columns - 1) as f64 * lng_step_deg >= bbox.lng_max {
            columns -= 1;
        }
        Some(GridRow {
            lat_min,
            lat_max,
            lng_step_deg,
            columns,
        })
    })
}

/// Exact number of cells `generate_initial_cells` produces for a bbox and step (no cap applied).
pub fn count_initial_cells(bbox: BoundingBox, step_km: f64) -> usize {
    initial_grid_rows(bbox, step_km).fold(0usize, |total, row| total.saturating_add(row.columns))
}

/// Approximate bbox size in km (width measured at the latitude closest to the equator).
pub fn bbox_dimensions_km(bbox: BoundingBox) -> (f64, f64) {
    let nearest_equator_lat = if bbox.lat_min <= 0.0 && bbox.lat_max >= 0.0 {
        0.0
    } else if bbox.lat_min.abs() < bbox.lat_max.abs() {
        bbox.lat_min
    } else {
        bbox.lat_max
    };
    let max_cos = nearest_equator_lat.to_radians().cos().abs().max(1e-9);
    let width_km = (bbox.lng_max - bbox.lng_min) * KM_PER_DEGREE * max_cos;
    let height_km = (bbox.lat_max - bbox.lat_min) * KM_PER_DEGREE;
    (width_km, height_km)
}

/// Smallest step (searched by bisection on the real cell count) that keeps the grid within `max_cells`.
pub fn recommended_step_for_cap(bbox: BoundingBox, current_step_km: f64, max_cells: usize) -> f64 {
    let (width_km, height_km) = bbox_dimensions_km(bbox);
    let mut low = current_step_km;
    let mut high = width_km.max(height_km).max(current_step_km);
    while count_initial_cells(bbox, high) > max_cells {
        high *= 2.0;
    }
    for _ in 0..48 {
        let mid = (low + high) / 2.0;
        if count_initial_cells(bbox, mid) > max_cells {
            low = mid;
        } else {
            high = mid;
        }
    }
    high
}

pub fn generate_initial_cells(
    bbox: BoundingBox,
    step_km: f64,
    limits: GridLimits,
) -> std::result::Result<Vec<GridCell>, ScrapeError> {
    let estimate = validate_and_estimate_grid(bbox, step_km, limits)?;
    if estimate.initial_cells > limits.max_initial_cells {
        let recommended = estimate.recommended_min_step_km.unwrap_or(step_km);
        return Err(ScrapeError {
            kind: crate::google_maps_jobs::types::ScrapeErrorKind::InvalidInput,
            message: format!(
                "grid requires {} initial cells, exceeding cap {}; use step_km >= {:.3}",
                estimate.initial_cells, limits.max_initial_cells, recommended
            ),
            retryable: false,
        });
    }

    let mut cells = Vec::with_capacity(estimate.initial_cells);
    for row in initial_grid_rows(bbox, step_km) {
        for column in 0..row.columns {
            if cells.len() >= limits.max_initial_cells {
                return Err(ScrapeError {
                    kind: crate::google_maps_jobs::types::ScrapeErrorKind::InvalidInput,
                    message: "generated grid exceeded the validated initial cell cap".to_string(),
                    retryable: false,
                });
            }
            let lng_min = bbox.lng_min + column as f64 * row.lng_step_deg;
            let lng_max = (lng_min + row.lng_step_deg).min(bbox.lng_max);
            cells.push(grid_cell_from_bbox(
                BoundingBox {
                    lat_min: row.lat_min,
                    lat_max: row.lat_max,
                    lng_min,
                    lng_max,
                },
                0,
                None,
                0,
            ));
        }
    }

    Ok(cells)
}

pub fn subdivide_cell(cell: &GridCell, new_unique_ratio: f64) -> Vec<GridCell> {
    let lat_mid = (cell.bbox.lat_min + cell.bbox.lat_max) / 2.0;
    let lng_mid = (cell.bbox.lng_min + cell.bbox.lng_max) / 2.0;
    let low_yield_levels = if new_unique_ratio.is_finite() && new_unique_ratio < 0.05 {
        cell.consecutive_low_yield_levels.saturating_add(1)
    } else {
        0
    };
    let depth = cell.depth.saturating_add(1);

    [
        BoundingBox {
            lat_min: lat_mid,
            lat_max: cell.bbox.lat_max,
            lng_min: cell.bbox.lng_min,
            lng_max: lng_mid,
        },
        BoundingBox {
            lat_min: lat_mid,
            lat_max: cell.bbox.lat_max,
            lng_min: lng_mid,
            lng_max: cell.bbox.lng_max,
        },
        BoundingBox {
            lat_min: cell.bbox.lat_min,
            lat_max: lat_mid,
            lng_min: cell.bbox.lng_min,
            lng_max: lng_mid,
        },
        BoundingBox {
            lat_min: cell.bbox.lat_min,
            lat_max: lat_mid,
            lng_min: lng_mid,
            lng_max: cell.bbox.lng_max,
        },
    ]
    .into_iter()
    .map(|bbox| grid_cell_from_bbox(bbox, depth, cell.parent_unit_id.clone(), low_yield_levels))
    .collect()
}

pub fn should_subdivide(
    cell: &GridCell,
    raw_listing_count: usize,
    saturation_threshold: usize,
    reached_end: bool,
    new_unique_ratio: f64,
    total_cells: usize,
    limits: GridLimits,
) -> bool {
    if reached_end && raw_listing_count < saturation_threshold {
        return false;
    }
    if cell.depth >= limits.max_depth || total_cells.saturating_add(4) > limits.max_total_cells {
        return false;
    }
    if cell_min_dimension_m(cell) <= limits.min_cell_size_m {
        return false;
    }
    let low_yield_levels = if new_unique_ratio.is_finite() && new_unique_ratio < 0.05 {
        cell.consecutive_low_yield_levels.saturating_add(1)
    } else {
        0
    };
    if low_yield_levels >= 2 {
        return false;
    }
    // The zoom steps are not exact halves, so requiring the child to reach a
    // higher zoom left saturated cells in some radius bands unsplit (B7). A cell
    // may split until it is already viewed at the finest zoom Maps offers.
    radius_to_zoom(cell.radius_m) < MAX_GRID_ZOOM
        && !subdivide_cell(cell, new_unique_ratio).is_empty()
}

/// Reports a cell that satisfies every subdivision condition except the
/// total-cell budget.
///
/// `should_subdivide` collapses "this cell does not need splitting" and "this
/// cell needs splitting but there is no budget left" into one `false`, which
/// let a job report a clean completion over truncated coverage. This separates
/// the second case so the runner can pause as `paused_limit_reached`.
#[allow(clippy::too_many_arguments)]
pub fn grid_budget_exhausted(
    cell: &GridCell,
    raw_listing_count: usize,
    saturation_threshold: usize,
    reached_end: bool,
    new_unique_ratio: f64,
    total_cells: usize,
    limits: GridLimits,
) -> bool {
    if should_subdivide(
        cell,
        raw_listing_count,
        saturation_threshold,
        reached_end,
        new_unique_ratio,
        total_cells,
        limits,
    ) {
        return false;
    }
    if total_cells.saturating_add(4) <= limits.max_total_cells {
        return false;
    }
    let unbounded = GridLimits {
        max_total_cells: usize::MAX,
        ..limits
    };
    should_subdivide(
        cell,
        raw_listing_count,
        saturation_threshold,
        reached_end,
        new_unique_ratio,
        0,
        unbounded,
    )
}

fn grid_cell_from_bbox(
    bbox: BoundingBox,
    depth: u8,
    parent_unit_id: Option<String>,
    consecutive_low_yield_levels: u8,
) -> GridCell {
    let center_lat = (bbox.lat_min + bbox.lat_max) / 2.0;
    let center_lng = (bbox.lng_min + bbox.lng_max) / 2.0;
    let step_lat_deg = bbox.lat_max - bbox.lat_min;
    let step_lng_deg = bbox.lng_max - bbox.lng_min;
    let height_m = step_lat_deg * 111_000.0;
    let width_m = step_lng_deg * 111_000.0 * center_lat.to_radians().cos().abs();
    let radius_m = (height_m / 2.0).hypot(width_m / 2.0);
    GridCell {
        bbox,
        center_lat,
        center_lng,
        step_lat_deg,
        step_lng_deg,
        radius_m,
        depth,
        parent_unit_id,
        consecutive_low_yield_levels,
    }
}

fn cell_min_dimension_m(cell: &GridCell) -> f64 {
    let height_m = cell.step_lat_deg * 111_000.0;
    let width_m = cell.step_lng_deg * 111_000.0 * cell.center_lat.to_radians().cos().abs();
    height_m.min(width_m)
}

impl GridCell {
    pub fn subdivide(&self) -> Vec<GridCell> {
        subdivide_cell(self, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generate_grid(bbox: BoundingBox, step_km: f64) -> Vec<GridCell> {
        generate_initial_cells(bbox, step_km, GridLimits::default()).unwrap_or_default()
    }

    #[test]
    fn default_grid_limits_match_the_approved_safety_budget() {
        let limits = GridLimits::default();

        assert_eq!(limits.max_depth, 6);
        assert!((limits.min_cell_size_m - 250.0).abs() < f64::EPSILON);
        assert_eq!(limits.max_initial_cells, 10_000);
        assert_eq!(limits.max_total_cells, 25_000);
    }

    #[test]
    fn test_grid_generation_and_subdivision() {
        let bbox = BoundingBox {
            lat_min: 10.0,
            lat_max: 10.2,
            lng_min: 106.0,
            lng_max: 106.2,
        };
        let cells = generate_grid(bbox, 10.0);
        assert!(!cells.is_empty(), "Should generate at least one cell");

        let cell = &cells[0];
        let sub_cells = cell.subdivide();
        assert_eq!(sub_cells.len(), 4, "Subdivision should yield 4 quadrants");

        for child in sub_cells {
            assert_eq!(child.depth, cell.depth + 1);
            assert!((child.step_lat_deg - cell.step_lat_deg / 2.0).abs() < 1e-12);
            assert!((child.step_lng_deg - cell.step_lng_deg / 2.0).abs() < 1e-12);
            assert!(child.radius_m < cell.radius_m);
        }
    }

    #[test]
    fn clips_last_cell_to_bbox_and_keeps_center_inside() {
        let bbox = BoundingBox {
            lat_min: 10.0,
            lat_max: 10.025,
            lng_min: 106.0,
            lng_max: 106.025,
        };
        let cells = generate_initial_cells(
            bbox,
            2.0,
            GridLimits {
                max_depth: 6,
                min_cell_size_m: 20.0,
                max_initial_cells: 100,
                max_total_cells: 1_000,
            },
        )
        .unwrap();

        assert!(cells.iter().any(|cell| cell.bbox.lat_max == bbox.lat_max));
        assert!(cells.iter().any(|cell| cell.bbox.lng_max == bbox.lng_max));
        for cell in cells {
            assert!(cell.bbox.lat_min >= bbox.lat_min);
            assert!(cell.bbox.lat_max <= bbox.lat_max);
            assert!(cell.bbox.lng_min >= bbox.lng_min);
            assert!(cell.bbox.lng_max <= bbox.lng_max);
            assert!((cell.bbox.lat_min..=cell.bbox.lat_max).contains(&cell.center_lat));
            assert!((cell.bbox.lng_min..=cell.bbox.lng_max).contains(&cell.center_lng));
        }
    }

    /// Spec §11.3 requires that reaching a safety budget pauses the job rather
    /// than silently truncating coverage. Distinguishing "not saturated" from
    /// "would subdivide but the budget is gone" is what makes that possible.
    #[test]
    fn budget_exhaustion_is_distinguishable_from_an_unsaturated_cell() {
        let limits = GridLimits {
            max_depth: 6,
            min_cell_size_m: 250.0,
            max_initial_cells: 100,
            max_total_cells: 1_000,
        };
        let cell = generate_initial_cells(
            BoundingBox {
                lat_min: 10.0,
                lat_max: 10.2,
                lng_min: 106.0,
                lng_max: 106.2,
            },
            10.0,
            limits,
        )
        .unwrap()
        .remove(0);

        assert!(
            grid_budget_exhausted(&cell, 100, 55, false, 1.0, 999, limits),
            "a saturated cell blocked only by the total-cell cap has exhausted the budget"
        );
        assert!(
            !grid_budget_exhausted(&cell, 3, 55, true, 1.0, 999, limits),
            "an exhausted low-yield cell is not a budget problem"
        );
        assert!(
            !grid_budget_exhausted(&cell, 100, 55, false, 1.0, 1, limits),
            "a cell that can still subdivide has not exhausted the budget"
        );
    }

    #[test]
    fn subdivision_stops_when_zoom_does_not_increase() {
        let cell = generate_initial_cells(
            BoundingBox {
                lat_min: 10.0,
                lat_max: 10.0001,
                lng_min: 106.0,
                lng_max: 106.0001,
            },
            1.0,
            GridLimits {
                max_depth: 6,
                min_cell_size_m: 1.0,
                max_initial_cells: 10,
                max_total_cells: 100,
            },
        )
        .unwrap()
        .remove(0);

        assert!(!should_subdivide(
            &cell,
            100,
            55,
            false,
            1.0,
            1,
            GridLimits {
                max_depth: 6,
                min_cell_size_m: 1.0,
                max_initial_cells: 10,
                max_total_cells: 100,
            },
        ));
    }

    /// B7: zoom thresholds are not exact halves, so a saturated cell with radius
    /// 2450 m used to have a child on the same zoom and never split.
    #[test]
    fn saturated_cell_with_radius_2450_subdivides() {
        // A square cell near the equator with radius = side / sqrt(2).
        let side_deg = 2450.0 * std::f64::consts::SQRT_2 / 111_000.0;
        let cell = grid_cell_from_bbox(
            BoundingBox {
                lat_min: 0.0,
                lat_max: side_deg,
                lng_min: 100.0,
                lng_max: 100.0 + side_deg,
            },
            0,
            None,
            0,
        );
        assert_eq!(radius_to_zoom(cell.radius_m), 15);
        assert_eq!(
            radius_to_zoom(subdivide_cell(&cell, 1.0)[0].radius_m),
            15,
            "fixture must hit the zoom plateau"
        );

        assert!(should_subdivide(
            &cell,
            120,
            55,
            false,
            1.0,
            1,
            GridLimits::default(),
        ));
    }

    #[test]
    fn subdivision_stops_after_two_low_yield_levels() {
        let mut cell = generate_initial_cells(
            BoundingBox {
                lat_min: 10.0,
                lat_max: 10.2,
                lng_min: 106.0,
                lng_max: 106.2,
            },
            10.0,
            GridLimits {
                max_depth: 6,
                min_cell_size_m: 20.0,
                max_initial_cells: 100,
                max_total_cells: 1_000,
            },
        )
        .unwrap()
        .remove(0);
        cell.consecutive_low_yield_levels = 1;

        assert!(!should_subdivide(
            &cell,
            100,
            55,
            false,
            0.01,
            1,
            GridLimits {
                max_depth: 6,
                min_cell_size_m: 20.0,
                max_initial_cells: 100,
                max_total_cells: 1_000,
            },
        ));
    }
}
