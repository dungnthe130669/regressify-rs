use anyhow::Result;
use walkdir::WalkDir;

use crate::{
    cli::args::TestArgs,
    common,
    helpers::OnceLockExtensions,
    test::test_suite::exec_test_suite,
    types::{TestSuiteFormat, TestSuiteInput},
};

pub async fn exec_test_suites(args: TestArgs) -> Result<()> {
    let TestArgs {
        test_suite: test_suites,
    } = args;
    let root_dir = common::ROOT_DIR.get_required()?;
    let visual_tests_dir = root_dir.join("visual_tests");

    // Find available test suites, where the file name ends with either `.tests.yaml`, `.tests.yml`, or `.tests.json`, ignoring case
    let mut available_test_suites = Vec::<TestSuiteInput>::new();
    for entry in WalkDir::new(&visual_tests_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let file_name = entry.file_name().to_string_lossy().to_lowercase();
        if file_name.ends_with(".tests.yaml")
            || file_name.ends_with(".tests.yml")
            || file_name.ends_with(".tests.json")
        {
            let (suite_name, ext) = file_name
                .rsplit_once(".tests.")
                .expect("File name should contain '.tests.'");

            available_test_suites.push(TestSuiteInput {
                name: suite_name.to_string(),
                path: entry.path().to_path_buf(),
                format: match ext {
                    "yaml" | "yml" => TestSuiteFormat::Yaml,
                    "json" => TestSuiteFormat::Json,
                    _ => panic!("Unsupported test suite format: {}", ext),
                },
            });
        }
    }

    let filtered_test_suites: Vec<TestSuiteInput> = if test_suites.is_empty()
        || test_suites.contains(&"all".to_string())
        || test_suites.contains(&"*".to_string())
    {
        available_test_suites
    } else {
        // let mut selected_suites = Vec::<TestSuiteInput>::new();
        // for suite in test_suites {
        //     let mut found = false;
        //     for available_suite in &available_test_suites {
        //         if glob::Pattern::new(suite)
        //             .map_err(|e| anyhow::anyhow!("Invalid glob pattern '{}': {}", suite, e))?
        //             .matches(&available_suite.name)
        //         {
        //             selected_suites.push(available_suite.clone());
        //             found = true;
        //         }
        //     }
        //     if !found {
        //         eprintln!("Warning: Test suite '{}' not found.", suite);
        //     }
        // }
        // selected_suites

        available_test_suites
            .into_iter()
            .filter(|ts| {
                test_suites.iter().any(|pattern| {
                    glob::Pattern::new(pattern)
                        .map_err(|e| anyhow::anyhow!("Invalid glob pattern '{}': {}", pattern, e))
                        .unwrap()
                        .matches(&ts.name)
                })
            })
            .collect()
    };

    for t in filtered_test_suites {
        let visual_tests_dir = &visual_tests_dir;
        match exec_test_suite(visual_tests_dir, &t).await {
            Ok(_) => eprintln!("Test suite '{}' completed successfully.", t.name),
            Err(e) => eprintln!("Error executing test suite '{}': {:?}", t.name, e),
        }
    }

    Ok(())
}
