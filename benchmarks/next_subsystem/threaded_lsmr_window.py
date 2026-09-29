"""Isolated scheduling experiment; never changes the working native source."""


def instrument(root):
    path = root / "legacy/fereg-core/src/memory.rs"
    text = path.read_text()
    before = """pub(crate) fn lsmr_rhs_window(rhs: usize, threads: usize) -> usize {
    let threads = threads.max(1);
"""
    after = before + """    // Diagnostic only: bound nested recurrences in medium threaded batches.
    if threads > 1 && (16..=64).contains(&rhs) && rhs > threads {
        return threads;
    }
"""
    assert text.count(before) == 1, "Inspect changed window policy before experimenting"
    path.write_text(text.replace(before, after))
