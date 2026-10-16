#[derive(Clone, Copy, Debug)]
pub(super) struct PixelRect {
    pub(super) x: usize,
    pub(super) y: usize,
    pub(super) width: usize,
    pub(super) height: usize,
}

impl PixelRect {
    pub(super) const fn right(self) -> usize {
        self.x + self.width
    }

    pub(super) const fn bottom(self) -> usize {
        self.y + self.height
    }

    pub(super) fn same_box(self, other: Self) -> bool {
        let overlap = self
            .right()
            .min(other.right())
            .saturating_sub(self.x.max(other.x))
            * self
                .bottom()
                .min(other.bottom())
                .saturating_sub(self.y.max(other.y));
        let smaller =
            (self.width * self.height).min(other.width * other.height);
        overlap * 5 >= smaller * 4
    }
}

/// Eight-connected components, including holes when called with inverted ink.
/// Analysis is bounded to 1600² pixels; no recursion or platform dependency.
pub(super) fn bounds(
    binary: &[u8],
    width: usize,
    height: usize,
) -> Vec<PixelRect> {
    let mut remaining = binary.to_vec();
    let mut queue = Vec::new();
    let mut parts = Vec::new();
    for index in 0..remaining.len() {
        if remaining[index] == 0 {
            continue;
        }
        remaining[index] = 0;
        queue.clear();
        queue.push(index);
        let (mut left, mut top) = (index % width, index / width);
        let (mut right, mut bottom) = (left, top);
        let mut next = 0;
        while next < queue.len() {
            let index = queue[next];
            next += 1;
            let (x, y) = (index % width, index / width);
            left = left.min(x);
            right = right.max(x);
            top = top.min(y);
            bottom = bottom.max(y);
            for row in y.saturating_sub(1)..=(y + 1).min(height - 1) {
                for col in x.saturating_sub(1)..=(x + 1).min(width - 1) {
                    let neighbor = row * width + col;
                    if remaining[neighbor] != 0 {
                        remaining[neighbor] = 0;
                        queue.push(neighbor);
                    }
                }
            }
        }
        parts.push(PixelRect {
            x: left,
            y: top,
            width: right - left + 1,
            height: bottom - top + 1,
        });
    }
    parts
}
