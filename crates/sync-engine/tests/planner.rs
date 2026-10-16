use foldersync_engine::{Action::*, Mode::*, classify};
#[test]
fn first_run_and_conflicts_preserve_originals() {
    assert_eq!(classify(TwoWay, Some("a"), None, None), CopyToB);
    assert_eq!(classify(TwoWay, None, Some("b"), None), CopyToA);
    assert_eq!(classify(TwoWay, Some("a"), Some("b"), None), Conflict);
    assert_eq!(classify(TwoWay, Some("a"), Some("a"), None), Unchanged);
}
#[test]
fn baseline_distinguishes_deletion_from_edit() {
    assert_eq!(classify(TwoWay, None, Some("base"), Some("base")), DeleteB);
    assert_eq!(classify(TwoWay, None, Some("edit"), Some("base")), Conflict);
    assert_eq!(
        classify(TwoWay, Some("edit"), Some("base"), Some("base")),
        CopyToB
    );
    assert_eq!(
        classify(TwoWay, Some("base"), Some("edit"), Some("base")),
        CopyToA
    );
    assert_eq!(
        classify(TwoWay, Some("x"), Some("y"), Some("base")),
        Conflict
    );
}
#[test]
fn directional_modes() {
    assert_eq!(classify(Copy, None, Some("b"), Some("b")), Unchanged);
    assert_eq!(classify(Mirror, None, Some("b"), None), DeleteB);
    assert_eq!(classify(Move, Some("a"), None, None), MoveToB);
    assert_eq!(classify(Move, Some("a"), Some("b"), None), Conflict);
    assert_eq!(classify(Copy, Some("a"), Some("b"), None), Conflict);
}
