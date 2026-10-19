//! Sample continuous size ranges; request only exact integer frame rates.

pub(super) fn sizes(info: rscam::ResolutionInfo) -> Vec<(u32, u32)> {
    match info {
        rscam::ResolutionInfo::Discretes(sizes) => {
            sizes.into_iter().take(128).collect()
        }
        rscam::ResolutionInfo::Stepwise { min, max, step } => {
            // Do not expand a potentially enormous continuous size grid.
            let mut sizes = vec![min, max];
            sizes.extend([
                (320, 240),
                (640, 480),
                (800, 600),
                (1024, 768),
                (1280, 720),
                (1280, 960),
                (1600, 1200),
                (1920, 1080),
                (2560, 1440),
                (3840, 2160),
            ]);
            sizes.retain(|&(w, h)| {
                w >= min.0
                    && h >= min.1
                    && w <= max.0
                    && h <= max.1
                    && (step.0 == 0 || (w - min.0).is_multiple_of(step.0))
                    && (step.1 == 0 || (h - min.1).is_multiple_of(step.1))
            });
            sizes.sort_unstable();
            sizes.dedup();
            sizes
        }
    }
}

pub(super) fn rates(info: &rscam::IntervalInfo) -> Vec<u32> {
    (1..=120)
        .filter(|&fps| match info {
            rscam::IntervalInfo::Discretes(intervals) => {
                intervals.iter().any(|&(n, d)| {
                    n != 0 && u64::from(n) * u64::from(fps) == u64::from(d)
                })
            }
            rscam::IntervalInfo::Stepwise { min, max, step } => {
                // 1/FPS must equal min + k*step, not a rounded frame rate.
                let [n, d, end_n, end_d, step_n, step_d, fps] =
                    [min.0, min.1, max.0, max.1, step.0, step.1, fps]
                        .map(u128::from);
                d != 0
                    && end_d != 0
                    && n * fps <= d
                    && end_n * fps >= end_d
                    && (step_n == 0
                        || (step_d != 0
                            && ((d - n * fps) * step_d)
                                .is_multiple_of(fps * d * step_n)))
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_never_round_fractional_rates_or_leave_size_grids() {
        assert_eq!(
            rates(&rscam::IntervalInfo::Discretes(vec![
                (1, 30),
                (2, 60),
                (1001, 30000),
                (0, 0),
                (1, 121),
            ])),
            vec![30]
        );
        assert_eq!(
            rates(&rscam::IntervalInfo::Stepwise {
                min: (1, 30),
                max: (1, 10),
                step: (1, 30),
            }),
            vec![10, 15, 30]
        );
        assert_eq!(
            sizes(rscam::ResolutionInfo::Stepwise {
                min: (640, 480),
                max: (1920, 1080),
                step: (16, 16),
            }),
            vec![(640, 480), (1024, 768), (1280, 720), (1280, 960)]
        );
        assert_eq!(
            rates(&rscam::IntervalInfo::Stepwise {
                min: (0, 0),
                max: (0, 0),
                step: (0, 0),
            }),
            [] as [u32; 0],
        );
    }
}
