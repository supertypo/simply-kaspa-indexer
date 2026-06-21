use vergen_git2::{Emitter, Git2};

fn main() {
    let git2 = Git2::builder().branch(true).commit_date(true).sha(true).describe(true, true, None).build();
    Emitter::default().add_instructions(&git2).unwrap().emit().unwrap();
}
