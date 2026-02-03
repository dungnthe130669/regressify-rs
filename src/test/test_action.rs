use anyhow::{Result, anyhow};
use chromiumoxide::Page;

use crate::{
    test::capture::save_screenshot,
    types::{
        CheckAction, ClickAction, FocusAction, GotoAction, HideAction, HoverAction, InputAction,
        PressAction, SaveAction, ScreenshotType, ScrollAction, SelectAction, TestAction,
        TestScenario, TestSuite, UncheckAction, Wait, WaitAction,
    },
};

pub async fn exec_test_action(
    page: &Page,
    test_suite: &TestSuite,
    scenario: &TestScenario,
    action: &TestAction,
) -> Result<()> {
    match action {
        TestAction::Check(CheckAction { check }) => {
            eprintln!("    Check element: {}", check);
            page.find_element(check).await?.click().await?;
        }

        TestAction::Click(ClickAction { click }) => {
            eprintln!("    Clicking element: {}", click);
            page.find_element(click).await?.click().await?;
        }

        TestAction::Focus(FocusAction { focus }) => {
            eprintln!("    Focusing element: {}", focus);
            page.find_element(focus).await?.focus().await?;
        }

        TestAction::Goto(GotoAction { goto }) => {
            eprintln!("    Navigating to URL: {}", goto);
            page.goto(goto).await?;
        }

        TestAction::Hide(HideAction { hide }) => {
            eprintln!("    Hiding element: {}", hide);
            // Inject CSS to hide the element
            let script = format!(
                "
                var style = document.createElement('style');
                style.type = 'text/css';
                style.innerHTML = '{} {{ visibility: hidden !important; }}';
                document.head.appendChild(style);
            ",
                hide
            );
            page.evaluate(script.as_str()).await?;
        }

        TestAction::Hover(HoverAction { hover }) => {
            eprintln!("    Hovering over element: {}", hover);
            page.find_element(hover).await?.hover().await?;
        }

        TestAction::Input(InputAction { input, value }) => {
            eprintln!("    Typing into element: {}", input);
            page.find_element(input).await?.type_str(value).await?;
        }

        TestAction::Press(PressAction { press, key }) => {
            eprintln!("    Pressing key '{}' on element: {}", key, press);
            page.find_element(press).await?.press_key(key).await?;
        }

        TestAction::Remove(remove) => {
            eprintln!("    Removing element: {}", remove.remove);
            // Set display to none to remove the element from view
            let script = format!(
                "
                var style = document.createElement('style');
                style.type = 'text/css';
                style.innerHTML = '{} {{ display: none !important; }}';
                document.head.appendChild(style);
            ",
                remove.remove
            );
            page.evaluate(script.as_str()).await?;
        }

        TestAction::Save(SaveAction { save }) => {
            eprintln!("    Saving screenshot: {}", save);

            save_screenshot(
                test_suite,
                scenario,
                page,
                ScreenshotType::Step(save.to_string()),
            )
            .await?
        }

        TestAction::Scroll(ScrollAction { scroll }) => {
            eprintln!("    Scrolling to element: {}", scroll);
            page.find_element(scroll).await?.scroll_into_view().await?;
        }

        TestAction::Select(SelectAction { select, value }) => {
            eprintln!("    Selecting value '{}' in element: {}", value, select);
            anyhow::bail!("Select action is not yet implemented");
        }

        TestAction::Uncheck(UncheckAction { uncheck }) => {
            eprintln!("    Unchecking element: {}", uncheck);
            let element = page.find_element(uncheck).await?;
            element.click().await?;
        }

        TestAction::Wait(WaitAction { wait, url }) => {
            if let Some(url) = url {
                eprintln!("    Waiting: {}", url);

                // Wait for navigation and ensure the URL matches
                page.wait_for_navigation().await?;
                let current_url = page
                    .url()
                    .await?
                    .ok_or_else(|| anyhow!("Failed to get current URL"))?;
                if &current_url != url {
                    anyhow::bail!("Expected URL '{}', but got '{}'", url, current_url);
                }
            }

            match wait {
                Wait::Number(milliseconds) => {
                    eprintln!("    Waiting for {} milliseconds", milliseconds);
                    tokio::time::sleep(std::time::Duration::from_millis(*milliseconds)).await;
                }
                Wait::String(selector) => {
                    eprintln!("    Waiting for element: {}", selector);
                    page.find_element(selector).await?;
                }
            }
        }
    }
    Ok(())
}
