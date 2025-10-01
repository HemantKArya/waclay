wit_bindgen::generate!({
    world: "guest",
    path: "wit/component.wit"
});

use exports::example::webscraper::exports::*;
use example::webscraper::{http, dom, scraper, pipeline};

struct Component;

impl Guest for Component {
    /// Main scraping function with complex nested types
    fn scrape_website(
        target: scraper::ScrapeTarget,
        pipeline_opt: Option<pipeline::DataPipeline>,
        custom_headers: Vec<http::HttpHeader>,
    ) -> Result<scraper::ScrapingResult, String> {
        // Log the scraping operation
        http::log_info(&format!("Starting scrape of: {}", target.url));
        http::log_info(&format!("Selectors count: {}", target.selectors.len()));
        http::log_info(&format!("Custom headers: {}", custom_headers.len()));
        
        // Build HTTP request with custom headers
        let mut headers = vec![
            http::HttpHeader {
                name: "User-Agent".to_string(),
                value: "WebScraperBot/1.0".to_string(),
            },
            http::HttpHeader {
                name: "Accept".to_string(),
                value: "text/html,application/xhtml+xml".to_string(),
            },
        ];
        headers.extend(custom_headers);
        
        // Create complex HTTP request
        let request = http::HttpRequest {
            method: http::HttpMethod::Get,
            url: target.url.clone(),
            headers,
            cookies: vec![
                http::Cookie {
                    name: "session".to_string(),
                    value: "abc123xyz".to_string(),
                    domain: Some("example.com".to_string()),
                    path: Some("/".to_string()),
                    expires: Some(1735689600000),
                    max_age: Some(3600),
                    secure: true,
                    http_only: true,
                    same_site: Some(http::SameSitePolicy::Lax),
                },
            ],
            body: None,
            timeout_ms: Some(30000),
            follow_redirects: true,
            max_redirects: Some(5),
        };
        
        // Make HTTP request through host
        let response = match http::make_request(&request) {
            Ok(resp) => {
                http::log_info(&format!("Response status: {}", resp.status.code));
                resp
            }
            Err(e) => {
                http::log_error(&format!("Request failed: {}", e));
                return Err(format!("HTTP request failed: {}", e));
            }
        };
        
        // Parse HTML if we got text content
        let mut extracted_data = Vec::new();
        
        if let Some(content) = &response.content {
            if let http::ResponseData::Text(html) = &content.data {
                // Parse HTML to DOM
                match dom::parse_html(&html) {
                    Ok(dom_elements) => {
                        http::log_info(&format!("Parsed {} root DOM elements", dom_elements.len()));
                        
                        // Extract data using selectors
                        for (idx, selector) in target.selectors.iter().enumerate() {
                            let field_name = if idx < target.required_fields.len() {
                                target.required_fields[idx].clone()
                            } else {
                                format!("field_{}", idx)
                            };
                            
                            // Query elements with selector
                            for root in &dom_elements {
                                match dom::query_selector(root, &selector) {
                                    Ok(matches) => {
                                        for element in matches {
                                            // Extract various types of data
                                            let value = extract_element_value(&element);
                                            
                                            extracted_data.push(scraper::ExtractedData {
                                                field_name: field_name.clone(),
                                                value,
                                                source_url: target.url.clone(),
                                                xpath: Some(build_xpath(&element)),
                                                confidence: 0.95,
                                            });
                                        }
                                    }
                                    Err(e) => {
                                        http::log_error(&format!("Selector query failed: {}", e));
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        http::log_error(&format!("HTML parsing failed: {}", e));
                    }
                }
            }
        }
        
        // Apply pipeline transforms if provided
        if let Some(pipe) = pipeline_opt {
            http::log_info(&format!("Applying pipeline: {}", pipe.name));
            match Self::process_data(extracted_data.clone(), pipe) {
                Ok(transformed) => {
                    extracted_data = transformed;
                    http::log_info(&format!("Pipeline processing complete: {} items", extracted_data.len()));
                }
                Err(e) => {
                    http::log_error(&format!("Pipeline processing failed: {}", e));
                }
            }
        }
        
        // Build metadata
        let metadata = scraper::ScrapeMetadata {
            start_time: 1704067200000,
            end_time: 1704067205000,
            duration_ms: 5000,
            pages_visited: 1,
            elements_extracted: extracted_data.len() as u32,
            cache_hits: 0,
            retry_count: 0,
        };
        
        // Build comprehensive result
        let result = scraper::ScrapingResult {
            target,
            data: extracted_data,
            errors: vec![],
            metadata,
            has_related_results: false,
        };
        
        http::log_info("Scraping completed successfully");
        Ok(result)
    }
    
    /// Batch scraping with multiple targets
    fn scrape_batch(
        targets: Vec<scraper::ScrapeTarget>,
        shared_config: Option<pipeline::DataPipeline>,
    ) -> Result<Vec<Result<scraper::ScrapingResult, String>>, String> {
        http::log_info(&format!("Starting batch scrape of {} targets", targets.len()));
        
        let mut results = Vec::new();
        
        for (idx, target) in targets.into_iter().enumerate() {
            http::log_info(&format!("Processing target {}/{}", idx + 1, results.len() + 1));
            
            let result = Self::scrape_website(
                target,
                shared_config.clone(),
                vec![],
            );
            
            results.push(result);
        }
        
        http::log_info(&format!("Batch scraping completed: {} results", results.len()));
        Ok(results)
    }
    
    /// Get scraping statistics
    fn get_statistics() -> scraper::ScrapeStatistics {
        http::log_info("Generating statistics");
        
        scraper::ScrapeStatistics {
            total_requests: 100,
            successful_requests: 95,
            failed_requests: 5,
            total_data_extracted: 1250,
            average_response_time_ms: 450.5,
            cache_hit_rate: 0.35,
            error_breakdown: vec![
                (scraper::ErrorType::Network, 3),
                (scraper::ErrorType::Timeout, 2),
            ],
        }
    }
    
    /// Process data through pipeline
    fn process_data(
        data: Vec<scraper::ExtractedData>,
        pipeline_cfg: pipeline::DataPipeline,
    ) -> Result<Vec<scraper::ExtractedData>, String> {
        http::log_info(&format!("Processing {} items through pipeline: {}", data.len(), pipeline_cfg.name));
        
        let mut processed = data;
        
        // Apply each stage
        for stage in &pipeline_cfg.stages {
            http::log_info(&format!("Applying stage: {}", stage.name));
            
            // Apply transforms
            for transform in &stage.transforms {
                processed = apply_transform(&processed, transform);
            }
            
            // Apply filters
            for filter in &stage.filters {
                processed = apply_filter(&processed, filter);
            }
            
            // Apply validators
            for validator in &stage.validators {
                if let Err(e) = validate_data(&processed, validator) {
                    match pipeline_cfg.error_handling {
                        pipeline::ErrorHandlingStrategy::FailFast => return Err(e),
                        pipeline::ErrorHandlingStrategy::SkipErrors => continue,
                        _ => http::log_error(&format!("Validation warning: {}", e)),
                    }
                }
            }
        }
        
        http::log_info(&format!("Pipeline processing complete: {} items", processed.len()));
        Ok(processed)
    }
    
    /// Transform HTTP response to structured data
    fn transform_response(
        response: http::HttpResponse,
        selectors: Vec<dom::Selector>,
    ) -> Result<Vec<scraper::ExtractedData>, String> {
        http::log_info("Transforming HTTP response to structured data");
        
        let mut extracted = Vec::new();
        
        // Extract data from headers
        for header in &response.headers {
            extracted.push(scraper::ExtractedData {
                field_name: format!("header_{}", header.name),
                value: scraper::ExtractedValue::Text(header.value.clone()),
                source_url: "response://headers".to_string(),
                xpath: None,
                confidence: 1.0,
            });
        }
        
        // Extract timing information
        extracted.push(scraper::ExtractedData {
            field_name: "response_time".to_string(),
            value: scraper::ExtractedValue::Number(response.timing.total_time_ms as f64),
            source_url: "response://timing".to_string(),
            xpath: None,
            confidence: 1.0,
        });
        
        // Extract content if available
        if let Some(content) = &response.content {
            if let http::ResponseData::Text(html) = &content.data {
                match dom::parse_html(&html) {
                    Ok(elements) => {
                        for selector in selectors {
                            for root in &elements {
                                if let Ok(matches) = dom::query_selector(root, &selector) {
                                    for element in matches {
                                        extracted.push(scraper::ExtractedData {
                                            field_name: "selected_element".to_string(),
                                            value: extract_element_value(&element),
                                            source_url: "response://body".to_string(),
                                            xpath: Some(build_xpath(&element)),
                                            confidence: 0.9,
                                        });
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        http::log_error(&format!("Failed to parse HTML: {}", e));
                    }
                }
            }
        }
        
        http::log_info(&format!("Extracted {} data points from response", extracted.len()));
        Ok(extracted)
    }
}

// Helper functions

fn extract_element_value(element: &dom::DomElement) -> scraper::ExtractedValue {
    // Check if element contains images
    for attr in &element.attributes {
        if attr.name == "src" && (element.tag_name == "img" || element.tag_name == "image") {
            return scraper::ExtractedValue::Image(scraper::ImageData {
                url: attr.value.clone(),
                alt_text: element.attributes.iter()
                    .find(|a| a.name == "alt")
                    .map(|a| a.value.clone()),
                width: parse_dimension(&element, "width"),
                height: parse_dimension(&element, "height"),
                format: None,
            });
        }
    }
    
    // Check for URLs
    for attr in &element.attributes {
        if attr.name == "href" {
            return scraper::ExtractedValue::Url(attr.value.clone());
        }
    }
    
    // Check for nested structure
    if element.has_children && element.child_count > 0 {
        // Return structured data for children
        let mut child_data = Vec::new();
        for idx in 0..element.child_count.min(5) {  // Limit to 5 children
            child_data.push((
                format!("child_{}", idx),
                format!("element_{}", idx)
            ));
        }
        return scraper::ExtractedValue::Structured(child_data);
    }
    
    // Default to text content
    if let Some(text) = &element.text_content {
        // Try to parse as number
        if let Ok(num) = text.trim().parse::<f64>() {
            return scraper::ExtractedValue::Number(num);
        }
        // Try to parse as boolean
        if text.trim().eq_ignore_ascii_case("true") || text.trim().eq_ignore_ascii_case("false") {
            return scraper::ExtractedValue::Boolean(text.trim().eq_ignore_ascii_case("true"));
        }
        scraper::ExtractedValue::Text(text.clone())
    } else {
        scraper::ExtractedValue::Text(String::new())
    }
}

fn build_xpath(element: &dom::DomElement) -> String {
    let mut path = element.parent_path.join("/");
    if !path.is_empty() {
        path.push('/');
    }
    path.push_str(&element.tag_name);
    if let Some(id) = &element.id {
        path.push_str(&format!("[@id='{}']", id));
    }
    path
}

fn parse_dimension(element: &dom::DomElement, attr_name: &str) -> Option<u32> {
    element.attributes.iter()
        .find(|a| a.name == attr_name)
        .and_then(|a| a.value.parse().ok())
}

fn apply_transform(data: &[scraper::ExtractedData], transform: &pipeline::TransformOperation) -> Vec<scraper::ExtractedData> {
    let mut result = Vec::new();
    
    for item in data {
        let mut new_item = item.clone();
        
        match (&item.value, transform) {
            (scraper::ExtractedValue::Text(text), pipeline::TransformOperation::Trim) => {
                new_item.value = scraper::ExtractedValue::Text(text.trim().to_string());
            }
            (scraper::ExtractedValue::Text(text), pipeline::TransformOperation::Lowercase) => {
                new_item.value = scraper::ExtractedValue::Text(text.to_lowercase());
            }
            (scraper::ExtractedValue::Text(text), pipeline::TransformOperation::Uppercase) => {
                new_item.value = scraper::ExtractedValue::Text(text.to_uppercase());
            }
            _ => {}
        }
        
        result.push(new_item);
    }
    
    result
}

fn apply_filter(data: &[scraper::ExtractedData], filter: &pipeline::FilterRule) -> Vec<scraper::ExtractedData> {
    data.iter()
        .filter(|item| {
            if item.field_name != filter.field {
                return true;
            }
            
            match (&item.value, &filter.condition) {
                (scraper::ExtractedValue::Text(text), pipeline::FilterCondition::Contains(substr)) => {
                    text.contains(substr)
                }
                (scraper::ExtractedValue::Text(text), pipeline::FilterCondition::StartsWith(prefix)) => {
                    text.starts_with(prefix)
                }
                (scraper::ExtractedValue::Text(text), pipeline::FilterCondition::IsEmpty) => {
                    text.is_empty()
                }
                (scraper::ExtractedValue::Text(text), pipeline::FilterCondition::IsNotEmpty) => {
                    !text.is_empty()
                }
                _ => true,
            }
        })
        .cloned()
        .collect()
}

fn validate_data(data: &[scraper::ExtractedData], validator: &pipeline::ValidationRule) -> Result<(), String> {
    for item in data {
        if item.field_name != validator.field {
            continue;
        }
        
        match (&item.value, validator.rule_type) {
            (scraper::ExtractedValue::Text(text), pipeline::ValidationType::Required) => {
                if text.is_empty() {
                    return Err(validator.error_message.clone());
                }
            }
            (scraper::ExtractedValue::Text(_), pipeline::ValidationType::Url) => {
                if !matches!(&item.value, scraper::ExtractedValue::Url(_)) {
                    return Err(validator.error_message.clone());
                }
            }
            _ => {}
        }
    }
    
    Ok(())
}

export!(Component);
