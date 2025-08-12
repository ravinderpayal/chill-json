#[cfg(test)]
mod fuzzy_json_tests {
    use chill_json::{FuzzyJsonParser, FuzzyJsonParserBuilder};
    use serde_json::json;

    #[test]
    fn test_basic_parsing() {
        let parser = FuzzyJsonParser::new();
        let result: serde_json::Value = parser.parse(r#"{\n"name": "test"\n}"#).unwrap();
        assert_eq!(result, json!({"name": "test"}));
    }

    #[test]
    fn test_trailing_comma() {
        let parser = FuzzyJsonParser::new();
        let result: serde_json::Value = parser.parse(r#"{\n"name": "test",  \n  }"#).unwrap();
        assert_eq!(result, json!({"name": "test"}));
    }

    #[test]
    fn test_code_block_markers() {
        let parser = FuzzyJsonParser::new();
        let result: serde_json::Value = parser.parse(r#"\n```json{"name": "test"}```"#).unwrap();
        println!("Result: {:?}", result);
        assert_eq!(result, json!({"name": "test"}));
    }

    #[test]
    fn test_single_quotes() {
        let parser = FuzzyJsonParser::new();
        // use json5, will pass
        let result: serde_json::Value = parser.parse(r#"{'name':\n 'test'}"#).unwrap();
        assert_eq!(result, json!({"name": "test"}));
    }
    // we are still missing the case where there's '' quote right
    // after property without any colons
    #[test]
    fn test_mixed_quotes() {
        let parser = FuzzyJsonParser::new();
        // use json5, will pass
        let result: serde_json::Value = parser
            .parse(r#"{'name':\n 'test', \n "hello": 'cat'}"#)
            .unwrap();
        assert_eq!(result, json!({"name": "test", "hello": "cat"}));
    }

    #[test]
    fn test_builder_pattern() {
        let parser = FuzzyJsonParserBuilder::new()
            .with_trailing_commas(true)
            .with_single_quotes(true)
            .strict_mode(false)
            .build();

        let result: serde_json::Value = parser.parse(r#"{'name': 'test',}"#).unwrap();
        assert_eq!(result, json!({"name": "test"}));
    }


    #[test]
    fn test_builder_pattern_2() {
        let parser = FuzzyJsonParserBuilder::new()
            .with_trailing_commas(true)
            .with_single_quotes(true)
            .strict_mode(false)
            .build();

        let result: serde_json::Value = parser.parse(r#"{"name": "test",}"#).unwrap();
        assert_eq!(result, json!({"name": "test"}));
    }

    #[test]
    fn test_truncated_string() {
        let parser = FuzzyJsonParser::new();

        // Truncated in middle of string value
        let truncated = r#"{"name": "OpenAI is an AI research"#;
        let result: serde_json::Value = parser.parse(truncated).unwrap();
        assert_eq!(result["name"], "OpenAI is an AI research");
    }

    #[test]
    fn test_truncated_object() {
        let parser = FuzzyJsonParser::new();

        // Truncated object missing closing brace
        let truncated = r#"{"name": "test", "active": true"#;
        let result: serde_json::Value = parser.parse(truncated).unwrap();
        assert_eq!(result["name"], "test");
        assert_eq!(result["active"], true);
    }

    #[test]
    fn test_truncated_array() {
        let parser = FuzzyJsonParser::new();

        // Truncated array missing closing bracket
        let truncated = r#"{"items": [1, 2, 3"#;
        let result: serde_json::Value = parser.parse(truncated).unwrap();
        assert_eq!(result["items"], json!([1, 2, 3]));
    }

    #[test]
    fn test_truncated_nested() {
        let parser = FuzzyJsonParser::new();

        // Deeply nested truncation
        let truncated = r#"{"user": {"profile": {"name": "John", "settings": {"theme": "dark""#;
        let result: serde_json::Value = parser.parse(truncated).unwrap();
        assert_eq!(result["user"]["profile"]["name"], "John");
        assert_eq!(result["user"]["profile"]["settings"]["theme"], "dark");
    }

    #[test]
    fn test_truncated_incomplete_property() {
        let parser = FuzzyJsonParser::new();

        // Truncated after property name
        let truncated = r#"{"name": "test", "incomplete""#;
        let result: serde_json::Value = parser.parse(truncated).unwrap();
        assert_eq!(result["name"], "test");
        assert_eq!(result["incomplete"], 0);
    }

    // numbered key
    // literal or number or string right after property, skipping the colon
    // test without quotes
    // there shouldn't be object right after property or colon

    #[test]
    fn test_truncated_with_incomplete_str_value_at_end() {
        let parser = FuzzyJsonParser::new();

        // Truncated after property name
        let truncated = r#"{"name": "test", "incomplete": "dfdf"#;
        let result: serde_json::Value = parser.parse(truncated).unwrap();
        assert_eq!(result["name"], "test");
        assert_eq!(result["incomplete"], "dfdf");
    }

    #[test]
    fn test_truncated_with_incomplete_number_property_at_end() {
        let parser = FuzzyJsonParser::new();

        // Truncated after property name
        let truncated = r#"{"name": "test", "incomplete": null, 0:"#;
        let result: serde_json::Value = parser.parse(truncated).unwrap();
        assert_eq!(result["name"], "test");
        assert_eq!(result["incomplete"], serde_json::Value::Null);
    }

    #[test]
    fn test_truncated_with_complete_str_value_at_end() {
        let parser = FuzzyJsonParser::new();

        // Truncated after property name
        let truncated = r#"{"name": "test", "incomplete": "dfdf""#;
        let result: serde_json::Value = parser.parse(truncated).unwrap();
        assert_eq!(result["name"], "test");
        assert_eq!(result["incomplete"], "dfdf");
    }

    #[test]
    fn test_truncated_after_colon() {
        let parser = FuzzyJsonParser::new();

        // Truncated right after colon
        let truncated = r#"{"name": "test", "value":"#;
        let result: serde_json::Value = parser.parse(truncated).unwrap();
        assert_eq!(result["name"], "test");
        assert!(result["value"] == 0);
    }

    #[test]
    fn test_truncated_array_with_trailing_comma() {
        let parser = FuzzyJsonParser::new();

        // Array with trailing comma then truncation
        let truncated = r#"{"items": [1, 2, 3,"#;
        let result: serde_json::Value = parser.parse(truncated).unwrap();
        assert_eq!(result["items"], json!([1, 2, 3]));
    }

    #[test]
    fn test_mixed_truncation_scenarios() {
        let parser = FuzzyJsonParser::new();

        // Complex truncation with multiple issues
        let truncated = r#"{"users": [{"name": "Alice", "age": 30}, {"name": "Bob"#;
        let result: serde_json::Value = parser.parse(truncated).unwrap();
        assert_eq!(result["users"][0]["name"], "Alice");
        assert_eq!(result["users"][0]["age"], 30);
        assert_eq!(result["users"][1]["name"], "Bob");
    }

    /*
     * useless test
    #[test]
    fn test_aggressive_repair_disabled() {
        let parser = FuzzyJsonParserBuilder::new()
            .aggressive_truncation_repair(false)
            .build();

        let truncated = r#"{"name": "test""#;
        // Should fail when aggressive repair is disabled
        assert!(parser
            .parse_with_auto_close::<serde_json::Value>(truncated)
            .is_err());
    }*/

    #[test]
    fn test_truncated_with_code_blocks() {
        let parser = FuzzyJsonParser::new();

        // LLM response with code blocks that gets truncated
        let truncated = r#"```json
{"name": "OpenAI", "type": "company", "founded": 2015"#;
        let result: serde_json::Value = parser.parse(truncated).unwrap();
        assert_eq!(result["name"], "OpenAI");
        assert_eq!(result["type"], "company");
        assert_eq!(result["founded"], 2015);
    }

    #[test]
    fn test_json_locked_in_json_code_formating() {
        let parser = FuzzyJsonParser::new();

        // LLM response with code blocks that gets truncated
        let json_str = "```json\n{\n\"is_global\": true,\n\"about_summary\": \"Bain & Company is a global management consulting firm that advises public, private, and nonprofit organizations on critical issues. Their specialties include corporate strategy, mergers & acquisitions, private equity, digital transformation, operations, customer experience, organizational effectiveness, and sustainability.\",\n\"size\": \"large\",\n\"is_publicly_listed\": false,\n\"last_year_revenue\": 6300000000,\n\"latest_head_count\": 19000,\n\"inception_year\": 1973,\n\"legal_name\": \"BAIN & COMPANY, INC.\",\n\"past_names\": [\"SUNAPEE SECURITIES, INC.\", \"BAIN & COMPANY, INC. UNITED KINGDOM\"],\n\"headquarter\": \"Boston, Massachusetts, U.S.\",\n\"office_locations\": [\"Boston\", \"New York\", \"London\", \"Paris\", \"Munich\", \"Tokyo\", \"Shanghai\", \"Sydney\", \"Singapore\", \"Dubai\", \"Johannesburg\", \"Chicago\", \"San Francisco\", \"Atlanta\", \"Dallas\", \"Houston\", \"Los Angeles\", \"Seattle\", \"Toronto\", \"Mexico City\", \"São Paulo\", \"Buenos Aires\", \"Copenhagen\", \"Frankfurt\", \"Helsinki\", \"Istanbul\", \"Madrid\", \"Milan\", \"Oslo\", \"Rome\", \"Stockholm\", \"Vienna\", \"Warsaw\", \"Zurich\", \"Beijing\", \"Bengaluru\", \"Ho Chi Minh City\", \"Hong Kong\", \"Jakarta\", \"Kuala Lumpur\", \"Manila\", \"Melbourne\", \"Mumbai\", \"New Delhi\", \"Perth\", \"Seoul\", \"Washington, DC\", \"Austin\", \"Denver\", \"Minneapolis\", \"Monterrey\", \"Montreal\", \"Rio de Janeiro\", \"Santiago\", \"Silicon Valley\", \"Bogota\", \"Athens\", \"Berlin\", \"Brussels\", \"Kyiv\", \"Doha\", \"Riyadh\"],\n\"industry\": \"Management Consulting\",\n\"sector\": \"Professional Services\",\n\"sub_sector\": \"Strategy and Management Consulting\",\n\"website\": \"www.bain.com\",\n\"is_b2b\": true,\n\"is_b2c\": false,\n\"is_product_company\": false,\n\"is_services_company\": true\n}\n```";
        let result: serde_json::Value = parser.parse(json_str).unwrap();
        assert_eq!(result["legal_name"], "BAIN & COMPANY, INC.");
        // assert_eq!(result["type"], "company");
        // assert_eq!(result["founded"], 2015);
    }

    #[test]
    fn test_json_having_arbitrary_text_in_beginning() {
        let parser = FuzzyJsonParser::new();

        // LLM response with code blocks that gets truncated
        let json_str = "Here's your json response for Band & Company ```json\n{\n\"is_global\": true,\n\"about_summary\": \"Bain & Company is a global management consulting firm that advises public, private, and nonprofit organizations on critical issues. Their specialties include corporate strategy, mergers & acquisitions, private equity, digital transformation, operations, customer experience, organizational effectiveness, and sustainability.\",\n\"size\": \"large\",\n\"is_publicly_listed\": false,\n\"last_year_revenue\": 6300000000,\n\"latest_head_count\": 19000,\n\"inception_year\": 1973,\n\"legal_name\": \"BAIN & COMPANY, INC.\",\n\"past_names\": [\"SUNAPEE SECURITIES, INC.\", \"BAIN & COMPANY, INC. UNITED KINGDOM\"],\n\"headquarter\": \"Boston, Massachusetts, U.S.\",\n\"office_locations\": [\"Boston\", \"New York\", \"London\", \"Paris\", \"Munich\", \"Tokyo\", \"Shanghai\", \"Sydney\", \"Singapore\", \"Dubai\", \"Johannesburg\", \"Chicago\", \"San Francisco\", \"Atlanta\", \"Dallas\", \"Houston\", \"Los Angeles\", \"Seattle\", \"Toronto\", \"Mexico City\", \"São Paulo\", \"Buenos Aires\", \"Copenhagen\", \"Frankfurt\", \"Helsinki\", \"Istanbul\", \"Madrid\", \"Milan\", \"Oslo\", \"Rome\", \"Stockholm\", \"Vienna\", \"Warsaw\", \"Zurich\", \"Beijing\", \"Bengaluru\", \"Ho Chi Minh City\", \"Hong Kong\", \"Jakarta\", \"Kuala Lumpur\", \"Manila\", \"Melbourne\", \"Mumbai\", \"New Delhi\", \"Perth\", \"Seoul\", \"Washington, DC\", \"Austin\", \"Denver\", \"Minneapolis\", \"Monterrey\", \"Montreal\", \"Rio de Janeiro\", \"Santiago\", \"Silicon Valley\", \"Bogota\", \"Athens\", \"Berlin\", \"Brussels\", \"Kyiv\", \"Doha\", \"Riyadh\"],\n\"industry\": \"Management Consulting\",\n\"sector\": \"Professional Services\",\n\"sub_sector\": \"Strategy and Management Consulting\",\n\"website\": \"www.bain.com\",\n\"is_b2b\": true,\n\"is_b2c\": false,\n\"is_product_company\": false,\n\"is_services_company\": true\n}\n```";
        let result: serde_json::Value = parser.parse(json_str).unwrap();
        assert_eq!(result["legal_name"], "BAIN & COMPANY, INC.");
        // assert_eq!(result["type"], "company");
        // assert_eq!(result["founded"], 2015);
    }

    // #[test]
    // disabled //uncomment to enable
    // this case normally orginates when LLMs use sources to fetch information and often end up
    // mentioning those sources in the text before the useful JSON
    fn test_json_having_arbitrary_text_in_beginning_2() {
        let parser = FuzzyJsonParser::new();

        // LLM response with code blocks that gets truncated
        let json_str = "Here's your json response for Band & Company using source[1][2][3] ```json\n{\n\"is_global\": true,\n\"about_summary\": \"Bain & Company is a global management consulting firm that advises public, private, and nonprofit organizations on critical issues. Their specialties include corporate strategy, mergers & acquisitions, private equity, digital transformation, operations, customer experience, organizational effectiveness, and sustainability.\",\n\"size\": \"large\",\n\"is_publicly_listed\": false,\n\"last_year_revenue\": 6300000000,\n\"latest_head_count\": 19000,\n\"inception_year\": 1973,\n\"legal_name\": \"BAIN & COMPANY, INC.\",\n\"past_names\": [\"SUNAPEE SECURITIES, INC.\", \"BAIN & COMPANY, INC. UNITED KINGDOM\"],\n\"headquarter\": \"Boston, Massachusetts, U.S.\",\n\"office_locations\": [\"Boston\", \"New York\", \"London\", \"Paris\", \"Munich\", \"Tokyo\", \"Shanghai\", \"Sydney\", \"Singapore\", \"Dubai\", \"Johannesburg\", \"Chicago\", \"San Francisco\", \"Atlanta\", \"Dallas\", \"Houston\", \"Los Angeles\", \"Seattle\", \"Toronto\", \"Mexico City\", \"São Paulo\", \"Buenos Aires\", \"Copenhagen\", \"Frankfurt\", \"Helsinki\", \"Istanbul\", \"Madrid\", \"Milan\", \"Oslo\", \"Rome\", \"Stockholm\", \"Vienna\", \"Warsaw\", \"Zurich\", \"Beijing\", \"Bengaluru\", \"Ho Chi Minh City\", \"Hong Kong\", \"Jakarta\", \"Kuala Lumpur\", \"Manila\", \"Melbourne\", \"Mumbai\", \"New Delhi\", \"Perth\", \"Seoul\", \"Washington, DC\", \"Austin\", \"Denver\", \"Minneapolis\", \"Monterrey\", \"Montreal\", \"Rio de Janeiro\", \"Santiago\", \"Silicon Valley\", \"Bogota\", \"Athens\", \"Berlin\", \"Brussels\", \"Kyiv\", \"Doha\", \"Riyadh\"],\n\"industry\": \"Management Consulting\",\n\"sector\": \"Professional Services\",\n\"sub_sector\": \"Strategy and Management Consulting\",\n\"website\": \"www.bain.com\",\n\"is_b2b\": true,\n\"is_b2c\": false,\n\"is_product_company\": false,\n\"is_services_company\": true\n}\n```";

        let result: serde_json::Value = parser.parse(json_str).unwrap();

        println!("Result: {:?}", result);

        assert_eq!(result["legal_name"], "BAIN & COMPANY, INC.");
        // assert_eq!(result["type"], "company");
        // assert_eq!(result["founded"], 2015);
    }

    #[test]
    fn test_json_having_arbitrary_text_at_the_end() {
        let parser = FuzzyJsonParser::new();

        // LLM response with code blocks that gets truncated
        let json_str = "Here's your json response for Band & Company ```json\n{\n\"is_global\": true,\n\"about_summary\": \"Bain & Company is a global management consulting firm that advises public, private, and nonprofit organizations on critical issues. Their specialties include corporate strategy, mergers & acquisitions, private equity, digital transformation, operations, customer experience, organizational effectiveness, and sustainability.\",\n\"size\": \"large\",\n\"is_publicly_listed\": false,\n\"last_year_revenue\": 6300000000,\n\"latest_head_count\": 19000,\n\"inception_year\": 1973,\n\"legal_name\": \"BAIN & COMPANY, INC.\",\n\"past_names\": [\"SUNAPEE SECURITIES, INC.\", \"BAIN & COMPANY, INC. UNITED KINGDOM\"],\n\"headquarter\": \"Boston, Massachusetts, U.S.\",\n\"office_locations\": [\"Boston\", \"New York\", \"London\", \"Paris\", \"Munich\", \"Tokyo\", \"Shanghai\", \"Sydney\", \"Singapore\", \"Dubai\", \"Johannesburg\", \"Chicago\", \"San Francisco\", \"Atlanta\", \"Dallas\", \"Houston\", \"Los Angeles\", \"Seattle\", \"Toronto\", \"Mexico City\", \"São Paulo\", \"Buenos Aires\", \"Copenhagen\", \"Frankfurt\", \"Helsinki\", \"Istanbul\", \"Madrid\", \"Milan\", \"Oslo\", \"Rome\", \"Stockholm\", \"Vienna\", \"Warsaw\", \"Zurich\", \"Beijing\", \"Bengaluru\", \"Ho Chi Minh City\", \"Hong Kong\", \"Jakarta\", \"Kuala Lumpur\", \"Manila\", \"Melbourne\", \"Mumbai\", \"New Delhi\", \"Perth\", \"Seoul\", \"Washington, DC\", \"Austin\", \"Denver\", \"Minneapolis\", \"Monterrey\", \"Montreal\", \"Rio de Janeiro\", \"Santiago\", \"Silicon Valley\", \"Bogota\", \"Athens\", \"Berlin\", \"Brussels\", \"Kyiv\", \"Doha\", \"Riyadh\"],\n\"industry\": \"Management Consulting\",\n\"sector\": \"Professional Services\",\n\"sub_sector\": \"Strategy and Management Consulting\",\n\"website\": \"www.bain.com\",\n\"is_b2b\": true,\n\"is_b2c\": false,\n\"is_product_company\": false,\n\"is_services_company\": true\n}}\n``` Can I help you with something else as well?";
        let result: serde_json::Value = parser.parse(json_str).unwrap();
        assert_eq!(result["legal_name"], "BAIN & COMPANY, INC.");
        // assert_eq!(result["type"], "company");
        // assert_eq!(result["founded"], 2015);
    }

    // #[test]
    fn test_json_having_arbitrary_wrapper_1() {
        let parser = FuzzyJsonParser::new();

        // LLM response with code blocks that gets truncated
        let json_str = "```json\n{\n  \"is_global\": false,\n  \"about_summary\": \"\",\n  \"\"size\"\": null,\n  \"is_publicly_listed\": false,\n  \"last_year_revenue\": null,\n  \"latest_head_count\": null,\n  \"inception_year\": null,\n  \"legal_name\": \"Biz4Group LLC\",\n  \"past_names\": [],\n  \"headquarter\": \"\",\n  \"office_locations\": [],\n  \"industry\": \"\",\n  \"sector\": \"\",\n  \"sub_sector\": \"\",\n  \"website\": \"\",\n  \"is_b2b\": false,\n  \"is_b2c\": false,\n  \"is_product_company\": false,\n  \"is_services_company\": false\n}\n```";
        let result: serde_json::Value = parser.parse(json_str).unwrap();
        assert_eq!(result["legal_name"], "Biz4Group LLC");
        // assert_eq!(result["type"], "company");
        // assert_eq!(result["founded"], 2015);
    }

    //#[test]
    fn test_json_having_arbitrary_wrapper_2() {
        let parser = FuzzyJsonParser::new();

        // LLM response with code blocks that gets truncated
        let json_str = "```json\n{\n  \"is_global\": false,\n  \"about_summary\": \"\",\n  \"size\": \"small\",\n  \"is_publicly_listed\": false,\n  \"last_year_revenue\": undefined,\n  \"latest_head_count\": undefined,\n  \"inception_year\": undefined,\n  \"legal_name\": \"BIO PETROCLEAN INDIA\",\n  \"past_names\": [],\n  \"headquarter\": \"\",\n  \"office_locations\": [],\n  \"industry\": \"\",\n  \"sector\": \"\",\n  \"sub_sector\": \"\",\n  \"website\": \"\",\n  \"is_b2b\": false,\n  \"is_b2c\": false,\n  \"is_product_company\": false,\n  \"is_services_company\": false\n}\n```";
        let result: serde_json::Value = parser.parse(json_str).unwrap();
        assert_eq!(result["legal_name"], "Biz4Group LLC");
        // assert_eq!(result["type"], "company");
        // assert_eq!(result["founded"], 2015);
    }
    #[test]
    fn test_json_having_arbitrary_wrapper_3() {
        let parser = FuzzyJsonParser::new();

        // LLM response with code blocks that gets truncated
        let json_str = "```{is_global: true, about_summary: 'PricewaterhouseCoopers, Ernst & Young, and KPMG are multinational professional services firms providing audit, tax, and consulting services.', size: 'large', is_publicly_listed: false, industry: 'Professional Services', sector: 'Accounting', sub_sector: 'Audit & Assurance', website: 'https://www.pwc.com; https://www.ey.com; https://home.kpmg/xx/en/home.html', is_b2b: true, is_b2c: false, is_product_company: false, is_services_company: true}```";
        let result: serde_json::Value = parser.parse(json_str).unwrap();
        assert_eq!(result["is_global"], true);
        // assert_eq!(result["type"], "company");
        // assert_eq!(result["founded"], 2015);
    }


    #[test]
    fn test_json_having_arbitrary_wrapper_4() {
        let parser = FuzzyJsonParser::new();

        // LLM response with code blocks that gets truncated
        let json_str = "```{is_global: false, about_summary: 'Satya Aesthetics specializes in aesthetics treatments and beauty services.', size: 'small', is_publicly_listed: false, last_year_revenue: undefined, latest_head_count: undefined, inception_year: undefined, legal_name: undefined, past_names: [], headquarter: 'Local', office_locations: ['Local'], industry: 'Aesthetics', sector: 'Health & Wellness', sub_sector: 'Beauty Services', website: 'https://satyaaesthetics.com', is_b2b: false, is_b2c: true, is_product_company: false, is_services_company: true}```";
        let result: serde_json::Value = parser.parse(json_str).unwrap();
        assert_eq!(result["is_global"], false);
        // assert_eq!(result["type"], "company");
        // assert_eq!(result["founded"], 2015);
    }
}
