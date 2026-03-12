use subversion::{
    client::{CheckoutOptions, CommitOptions, Context, CopyOptions, MergeSourcesOptions},
    Depth, Revision,
};

/// Example demonstrating branch merging with merge_peg
///
/// This example shows how to:
/// 1. Create a repository and trunk
/// 2. Create a branch
/// 3. Make changes in the branch
/// 4. Merge changes back to trunk using merge_peg
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Branch Merge Example using merge_peg\n");

    // Create a temporary directory for our test
    let temp_dir = tempfile::tempdir()?;
    let repo_path = temp_dir.path().join("repo");
    let trunk_wc = temp_dir.path().join("trunk");
    let branch_wc = temp_dir.path().join("branch");

    // Create a repository
    println!("Creating repository...");
    subversion::repos::Repos::create(&repo_path)?;

    let mut ctx = Context::new()?;
    let trunk_url = format!("file://{}", repo_path.display());

    // Checkout trunk
    println!("Checking out trunk...");
    ctx.checkout(
        trunk_url.as_str(),
        &trunk_wc,
        &CheckoutOptions {
            peg_revision: Revision::Head,
            revision: Revision::Head,
            depth: Depth::Infinity,
            ignore_externals: false,
            allow_unver_obstructions: false,
        },
    )?;

    // Create initial file structure in trunk
    println!("Creating initial files...");
    let readme = trunk_wc.join("README.md");
    std::fs::write(&readme, "# My Project\n\nInitial version\n")?;

    let src_file = trunk_wc.join("main.rs");
    std::fs::write(
        &src_file,
        "fn main() {\n    println!(\"Hello, world!\");\n}\n",
    )?;

    ctx.add(&readme, &subversion::client::AddOptions::default())?;
    ctx.add(&src_file, &subversion::client::AddOptions::default())?;

    let mut rev1 = 0i64;
    ctx.commit(
        &[trunk_wc.to_str().unwrap()],
        &CommitOptions::default(),
        std::collections::HashMap::new(),
        None,
        &mut |info| {
            rev1 = info.revision().as_i64();
            println!("  Committed revision {}", rev1);
            Ok(())
        },
    )?;

    // Create a branch
    println!("\nCreating branch...");
    let branch_url = format!("{}/branch", repo_path.display());
    let mut copy_opts = CopyOptions::new();

    ctx.copy(
        &[(trunk_url.as_str(), Some(Revision::Head))],
        &branch_url,
        &mut copy_opts,
    )?;
    println!("  Branch created at: {}", branch_url);

    // Checkout the branch
    println!("\nChecking out branch...");
    let branch_url_obj = format!("file://{}", branch_url);
    ctx.checkout(
        branch_url_obj.as_str(),
        &branch_wc,
        &CheckoutOptions {
            peg_revision: Revision::Head,
            revision: Revision::Head,
            depth: Depth::Infinity,
            ignore_externals: false,
            allow_unver_obstructions: false,
        },
    )?;

    // Make changes in the branch
    println!("\nMaking changes in branch...");
    let branch_readme = branch_wc.join("README.md");
    std::fs::write(
        &branch_readme,
        "# My Project\n\nInitial version\n\n## New Feature\n\nAdded cool feature!\n",
    )?;

    let branch_src = branch_wc.join("main.rs");
    std::fs::write(
        &branch_src,
        "fn main() {\n    println!(\"Hello, world!\");\n    println!(\"New feature added!\");\n}\n",
    )?;

    let mut rev2 = 0i64;
    ctx.commit(
        &[branch_wc.to_str().unwrap()],
        &CommitOptions::default(),
        std::collections::HashMap::new(),
        None,
        &mut |info| {
            rev2 = info.revision().as_i64();
            println!("  Committed revision {} to branch", rev2);
            Ok(())
        },
    )?;

    // Now merge from branch to trunk
    println!("\n=== Merging branch to trunk ===\n");

    println!("1. Using automatic merge (empty ranges)...");
    let merge_opts = MergeSourcesOptions {
        ignore_mergeinfo: false,
        diff_ignore_ancestry: false,
        force_delete: false,
        record_only: false,
        dry_run: false,
        allow_mixed_rev: true,
        merge_options: None,
    };

    match ctx.merge_peg(
        &branch_url_obj,
        &[], // Empty ranges = automatic merge
        &Revision::Head,
        trunk_wc.to_str().unwrap(),
        Depth::Infinity,
        &merge_opts,
    ) {
        Ok(()) => {
            println!("  ✓ Merge completed successfully!");

            // Verify the merge
            let merged_readme = std::fs::read_to_string(&readme)?;
            if merged_readme.contains("New Feature") {
                println!("  ✓ Changes from branch are present in trunk");
            }
        }
        Err(e) => {
            eprintln!("  ✗ Merge failed: {}", e);
            println!("\n2. Trying with explicit revision range...");

            // Try with explicit revision range
            let ranges = vec![subversion::mergeinfo::MergeRange::new(
                subversion::Revnum::from(rev1 as u64),
                subversion::Revnum::from(rev2 as u64),
                true, // inheritable
            )];

            ctx.merge_peg(
                &branch_url_obj,
                &ranges,
                &Revision::Number(subversion::Revnum::from(rev2 as u64)),
                trunk_wc.to_str().unwrap(),
                Depth::Infinity,
                &merge_opts,
            )?;
            println!("  ✓ Merge with explicit range succeeded!");
        }
    }

    // Show the merged content
    println!("\n=== Merged Content ===\n");
    let merged_readme = std::fs::read_to_string(&readme)?;
    println!("README.md:");
    println!("{}", merged_readme);

    let merged_src = std::fs::read_to_string(&src_file)?;
    println!("\nmain.rs:");
    println!("{}", merged_src);

    // Commit the merge
    println!("\n=== Committing merge ===\n");
    ctx.commit(
        &[trunk_wc.to_str().unwrap()],
        &CommitOptions::default(),
        std::collections::HashMap::new(),
        None,
        &mut |info| {
            println!("  Merge committed as revision {}", info.revision().as_i64());
            Ok(())
        },
    )?;

    println!("\n=== Example completed successfully! ===");
    println!("\nThis example demonstrated:");
    println!("  • Creating a repository and trunk");
    println!("  • Creating a branch with copy");
    println!("  • Making changes in the branch");
    println!("  • Merging with merge_peg (automatic and explicit ranges)");
    println!("  • Committing the merge result");

    Ok(())
}
