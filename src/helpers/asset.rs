#![allow(dead_code)]

use anyhow::{Result, anyhow, bail};

pub trait AssetExtensions {
    fn remove_source_map(&self) -> Result<String>;
}

impl AssetExtensions for String {
    fn remove_source_map(&self) -> Result<String> {
        let output = self
            .lines()
            .filter(|l| {
                !l.starts_with("/*# sourceMappingURL=") && !l.starts_with("//# sourceMappingURL=")
            })
            .collect::<Vec<&str>>()
            .join("\n");

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_block_comment_source_map() {
        let input =
            "line 1\nline 2\n/*# sourceMappingURL=data:application/json;base64,123 */".to_string();
        let result = input.remove_source_map().unwrap();
        assert_eq!(result, "line 1\nline 2");
    }

    #[test]
    fn test_remove_line_comment_source_map() {
        let input =
            "line 1\nline 2\n//# sourceMappingURL=data:application/json;base64,123".to_string();
        let result = input.remove_source_map().unwrap();
        assert_eq!(result, "line 1\nline 2");
    }

    #[test]
    fn test_remove_multiple_source_maps() {
        let input =
            "line 1\n//# sourceMappingURL=map1\nline 2\n/*# sourceMappingURL=map2 */\nline 3"
                .to_string();
        let result = input.remove_source_map().unwrap();
        assert_eq!(result, "line 1\nline 2\nline 3");
    }

    #[test]
    fn test_no_source_map_present() {
        let input = "line 1\nline 2\nline 3".to_string();
        let result = input.remove_source_map().unwrap();
        assert_eq!(result, "line 1\nline 2\nline 3");
    }

    #[test]
    fn test_empty_string() {
        let input = "".to_string();
        let result = input.remove_source_map().unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_only_source_map_block_comment() {
        let input = "/*# sourceMappingURL=data:application/json;base64,123 */".to_string();
        let result = input.remove_source_map().unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_only_source_map_line_comment() {
        let input = "//# sourceMappingURL=data:application/json;base64,123".to_string();
        let result = input.remove_source_map().unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_source_map_with_url() {
        let input = "console.log('test');\n//# sourceMappingURL=bundle.js.map".to_string();
        let result = input.remove_source_map().unwrap();
        assert_eq!(result, "console.log('test');");
    }

    #[test]
    fn test_source_map_with_data_uri() {
        let input = "var x = 1;\n/*# sourceMappingURL=data:application/json;charset=utf-8;base64,eyJ2ZXJzaW9uIjozfQ== */".to_string();
        let result = input.remove_source_map().unwrap();
        assert_eq!(result, "var x = 1;");
    }

    #[test]
    fn test_similar_comments_not_removed() {
        let input =
            "line 1\n/* sourceMappingURL=test */\n//sourceMappingURL=test\nline 2".to_string();
        let result = input.remove_source_map().unwrap();
        assert_eq!(
            result,
            "line 1\n/* sourceMappingURL=test */\n//sourceMappingURL=test\nline 2"
        );
    }

    #[test]
    fn test_source_map_mid_line_not_removed() {
        let input = "code before //# sourceMappingURL=map".to_string();
        let result = input.remove_source_map().unwrap();
        assert_eq!(result, "code before //# sourceMappingURL=map");
    }

    #[test]
    fn test_multiple_consecutive_source_maps() {
        let input = "code\n//# sourceMappingURL=map1\n//# sourceMappingURL=map2\n/*# sourceMappingURL=map3 */".to_string();
        let result = input.remove_source_map().unwrap();
        assert_eq!(result, "code");
    }

    #[test]
    fn test_whitespace_before_source_map() {
        let input = "code\n  //# sourceMappingURL=map".to_string();
        let result = input.remove_source_map().unwrap();
        assert_eq!(result, "code\n  //# sourceMappingURL=map");
    }

    #[test]
    fn test_source_map_at_start() {
        let input = "//# sourceMappingURL=map\ncode line 1\ncode line 2".to_string();
        let result = input.remove_source_map().unwrap();
        assert_eq!(result, "code line 1\ncode line 2");
    }

    #[test]
    fn test_source_map_at_end_no_newline() {
        let input = "code\n//# sourceMappingURL=map".to_string();
        let result = input.remove_source_map().unwrap();
        assert_eq!(result, "code");
    }

    #[test]
    fn test_multiline_with_mixed_content() {
        let input = "function test() {\n  console.log('hello');\n}\n//# sourceMappingURL=bundle.js.map\n/*# sourceMappingURL=test */\nmore code".to_string();
        let result = input.remove_source_map().unwrap();
        assert_eq!(
            result,
            "function test() {\n  console.log('hello');\n}\nmore code"
        );
    }

    #[test]
    fn test_special_characters_in_source_map_url() {
        let input =
            "code\n//# sourceMappingURL=file:///path/to/map?foo=bar&baz=qux#anchor".to_string();
        let result = input.remove_source_map().unwrap();
        assert_eq!(result, "code");
    }

    #[test]
    fn test_result_is_ok() {
        let input = "test".to_string();
        let result = input.remove_source_map();
        assert!(result.is_ok());
    }

    #[test]
    fn test_complex_minified_code_with_source_map() {
        let input = "function test(){return 1;}var x=2;\n//# sourceMappingURL=bundle.min.js.map"
            .to_string();
        let result = input.remove_source_map().unwrap();
        assert_eq!(result, "function test(){return 1;}var x=2;");
    }

    #[test]
    fn test_javascript_with_inline_source_map() {
        let input = "const obj = {map: 1};\n/*# sourceMappingURL=data:... */".to_string();
        let result = input.remove_source_map().unwrap();
        assert_eq!(result, "const obj = {map: 1};");
    }

    #[test]
    fn test_only_whitespace() {
        let input = "   \n\t\n  ".to_string();
        let result = input.remove_source_map().unwrap();
        assert_eq!(result, "   \n\t\n  ");
    }

    #[test]
    fn test_case_sensitivity_exact_match_only() {
        let input = "code\n//# SourceMappingURL=map\n//#sourceMappingURL=map".to_string();
        let result = input.remove_source_map().unwrap();
        assert_eq!(
            result,
            "code\n//# SourceMappingURL=map\n//#sourceMappingURL=map"
        );
    }

    #[test]
    fn test_source_map_with_comments_around_it() {
        let input = "code\n// comment\n//# sourceMappingURL=map\n// another comment".to_string();
        let result = input.remove_source_map().unwrap();
        assert_eq!(result, "code\n// comment\n// another comment");
    }

    #[test]
    fn test_unicode_in_code_with_source_map() {
        let input = "console.log('你好');\n//# sourceMappingURL=map".to_string();
        let result = input.remove_source_map().unwrap();
        assert_eq!(result, "console.log('你好');");
    }

    #[test]
    fn test_long_source_map_url() {
        let long_url = "data:application/json;charset=utf-8;base64,eyJ2ZXJzaW9uIjozLCJzb3VyY2VzIjpbInNyYy9pbmRleC50cyJdLCJuYW1lcyI6WyJyZXN0Il0sIm1hcHBpbmdzIjoiQUFDQSxJQUFNLEtBQUssR0FBRyxDQUFFLENBQUUsQ0FBRSxDQUFFLENBQUUsQ0FBRSxDQUFFLENBQUUsQ0FBRSxDQUFFLENBQUU7QUFDdkMsSUFBSSxLQUFLLENBQUMsS0FBSyxHQUFHLENBQUMsRUFBRTtFQUNuQixLQUFLLENBQUMsS0FBSyxDQUFDLE9BQU8sQ0FBQyxVQUFBLElBQUk7SUFDdEIsT0FBQSxJQUFJLEdBQUcsRUFBRTtFQUFULENBQVMsQ0FBQyxDQUFDO0NBQ2Q7QUFDRCxJQUFNLElBQUksR0FBRyxLQUFLLENBQUMsQ0FBQyxDQUFDLENBQUM7QUFDdkIsT0FBTyxDQUFDLEdBQUcsQ0FBQyxJQUFJLEVBQUUsQ0FBQyxFQUFFLEVBQUUsQ0FBQyxDQUFDIn0=";
        let input = format!("var x = 1;\n//# sourceMappingURL={}", long_url);
        let result = input.remove_source_map().unwrap();
        assert_eq!(result, "var x = 1;");
    }

    #[test]
    fn test_source_map_lines_with_different_separators() {
        let input = "line 1\r\n//# sourceMappingURL=map1\nline 2".to_string();
        let result = input.remove_source_map().unwrap();
        assert!(result.contains("line 1"));
        assert!(result.contains("line 2"));
        assert!(!result.contains("sourceMappingURL"));
    }
}
