use super::{DesignSpec, Graph, Rng};

pub(super) fn codes(spec: &DesignSpec, rng: &mut Rng) -> Vec<Vec<u32>> {
    let mut codes = vec![vec![0; spec.rows]; spec.levels.len()];
    if spec.levels.len() < 2 {
        if let Some(first) = codes.first_mut() {
            for (i, code) in first.iter_mut().enumerate() {
                *code = (i % spec.levels[0]) as u32;
            }
        }
        return codes;
    }
    let edges = edges(spec.levels[0], spec.levels[1], spec.graph);
    for i in 0..spec.rows {
        let (a, b) = edges[i % edges.len()];
        codes[0][i] = a as u32;
        codes[1][i] = b as u32;
        for (j, column) in codes.iter_mut().enumerate().skip(2) {
            let levels = spec.levels[j];
            let level = if spec.graph == Graph::Disconnected && levels >= 2 {
                let split = levels / 2;
                if a < spec.levels[0] / 2 {
                    rng.next() as usize % split
                } else {
                    split + rng.next() as usize % (levels - split)
                }
            } else {
                rng.next() as usize % levels
            };
            column[i] = level as u32;
        }
    }
    codes
}

fn edges(left: usize, right: usize, graph: Graph) -> Vec<(usize, usize)> {
    if left < 2 || right < 2 {
        return (0..left)
            .flat_map(|a| (0..right).map(move |b| (a, b)))
            .collect();
    }
    match graph {
        Graph::DensePanel => (0..left)
            .flat_map(|a| (0..right).map(move |b| (a, b)))
            .collect(),
        Graph::AkmStrong => (0..4)
            .flat_map(|t| (0..left).map(move |a| (a, (a + t) % right)))
            .collect(),
        Graph::AkmWeak => {
            // Sixteen home observations per worker and one move to the next firm.
            let mut edges = Vec::new();
            for a in 0..left {
                edges.extend(std::iter::repeat_n((a, a % right), 16));
                edges.push((a, (a + 1) % right));
            }
            edges
        }
        Graph::Chain => {
            let width = left.max(right);
            (0..width)
                .flat_map(|i| {
                    let a = i * left / width;
                    let b = i * right / width;
                    [(a, b), (a, ((i + 1) * right / width).min(right - 1))]
                })
                .collect()
        }
        Graph::Barbell | Graph::Disconnected => {
            let mut edges = (0..left)
                .flat_map(|a| {
                    (0..right)
                        .filter_map(move |b| ((a < left / 2) == (b < right / 2)).then_some((a, b)))
                })
                .collect::<Vec<_>>();
            if graph == Graph::Barbell {
                edges.push((left / 2 - 1, right / 2));
            }
            edges
        }
    }
}
