mod bindings;

use anyhow::Result;
use bindings::*;
use wasm_component_layer::*;

struct WebScraperHost;

// Implement HTTP interface
impl HttpHost for WebScraperHost {
    fn make_request(&mut self, request: HttpRequest) -> Result<HttpResponse, String> {
        println!("🌐 [Host] HTTP Request:");
        println!("   Method: {:?}", request.method);
        println!("   URL: {}", request.url);
        println!("   Headers: {}", request.headers.len());
        println!("   Cookies: {}", request.cookies.len());
        println!("   Follow Redirects: {}", request.follow_redirects);
        if let Some(ref body) = request.body {
            match body {
                RequestBody::Text(t) => println!("   Body: Text ({} bytes)", t.len()),
                RequestBody::Json(j) => println!("   Body: JSON ({} bytes)", j.len()),
                RequestBody::Form(f) => println!("   Body: Form ({} fields)", f.len()),
                RequestBody::Binary(b) => println!("   Body: Binary ({} bytes)", b.len()),
                RequestBody::Empty => println!("   Body: Empty"),
            }
        }
        println!();

        // Simulate HTTP response with HTML content
        let html_content = r#"<!DOCTYPE html>
<html>
<head>
    <title>Sample E-commerce Page</title>
</head>
<body>
    <div class="container">
        <header id="main-header">
            <h1 class="site-title">Welcome to Example Shop</h1>
            <nav>
                <a href="/products">Products</a>
                <a href="/about">About</a>
                <a href="/contact">Contact</a>
            </nav>
        </header>
        
        <main class="product-list">
            <article class="product" data-id="101">
                <img src="/images/product1.jpg" alt="Product 1" width="300" height="300">
                <h2 class="product-name">Awesome Widget</h2>
                <p class="product-description">The best widget you'll ever own!</p>
                <span class="price" data-currency="USD">$29.99</span>
                <a href="/products/101" class="buy-button">Buy Now</a>
            </article>
            
            <article class="product" data-id="102">
                <img src="/images/product2.jpg" alt="Product 2" width="300" height="300">
                <h2 class="product-name">Super Gadget</h2>
                <p class="product-description">Technology at its finest.</p>
                <span class="price" data-currency="USD">$49.99</span>
                <a href="/products/102" class="buy-button">Buy Now</a>
            </article>
            
            <article class="product" data-id="103">
                <img src="/images/product3.jpg" alt="Product 3" width="300" height="300">
                <h2 class="product-name">Ultra Tool</h2>
                <p class="product-description">Everything you need in one place.</p>
                <span class="price" data-currency="USD">$79.99</span>
                <a href="/products/103" class="buy-button">Buy Now</a>
            </article>
        </main>
        
        <footer id="site-footer">
            <p class="copyright">© 2025 Example Shop. All rights reserved.</p>
            <div class="social-links">
                <a href="https://twitter.com/example">Twitter</a>
                <a href="https://facebook.com/example">Facebook</a>
            </div>
        </footer>
    </div>
</body>
</html>"#
            .to_string();

        Ok(HttpResponse {
            status: HttpStatus {
                code: 200,
                text: "OK".to_string(),
            },
            headers: vec![
                HttpHeader {
                    name: "content-type".to_string(),
                    value: "text/html; charset=utf-8".to_string(),
                },
                HttpHeader {
                    name: "content-length".to_string(),
                    value: html_content.len().to_string(),
                },
            ],
            cookies: vec![],
            content: Some(ResponseContent {
                content_type: "text/html; charset=utf-8".to_string(),
                data: ResponseData::Text(html_content),
                encoding: Some("utf-8".to_string()),
            }),
            redirect_chain: vec![],
            timing: ResponseTiming {
                dns_lookup_ms: 5,
                tcp_connect_ms: 10,
                tls_handshake_ms: Some(15),
                time_to_first_byte_ms: 50,
                total_time_ms: 100,
            },
        })
    }

    fn log_info(&mut self, message: String) {
        println!("ℹ️  [Guest Info] {}", message);
    }

    fn log_error(&mut self, message: String) {
        eprintln!("❌ [Guest Error] {}", message);
    }
}

// Implement DOM interface
impl DomHost for WebScraperHost {
    fn parse_html(&mut self, html: String) -> Result<Vec<DomElement>, String> {
        println!("📄 [Host] Parsing HTML ({} bytes)", html.len());

        // Return empty for now to test
        Ok(vec![])
    }

    fn query_selector(
        &mut self,
        root: DomElement,
        selector: Selector,
    ) -> Result<Vec<DomElement>, String> {
        println!(
            "🔍 [Host] Query Selector on <{}> with selector: {:?}",
            root.tag_name, selector
        );

        // Simulate query selection
        Ok(vec![root])
    }
}

// Implement Scraper interface (no functions to implement, but trait must be satisfied)
impl ScraperHost for WebScraperHost {}

// Implement Pipeline interface (no functions to implement, but trait must be satisfied)
impl PipelineHost for WebScraperHost {}

fn main() -> Result<()> {
    println!("║   🕷️  Web Scraper - Complex Nested Types Example   ║");

    let engine = Engine::new(wasmi_runtime_layer::Engine::default());
    let mut store = Store::new(&engine, WebScraperHost);

    println!("📦 Loading WebAssembly component...");
    let component_bytes = std::fs::read("examples/web_scraper/component/component.wasm")?;
    let component = Component::new(&engine, &component_bytes)?;
    println!("✅ Component loaded successfully\n");

    println!("🔗 Setting up host functions...");
    let mut linker = Linker::default();
    imports::register_http_host(&mut linker, &mut store)?;
    imports::register_dom_host(&mut linker, &mut store)?;
    imports::register_scraper_host(&mut linker, &mut store)?;
    imports::register_pipeline_host(&mut linker, &mut store)?;
    println!("✅ Host functions registered\n");

    println!("🚀 Instantiating component...");
    let instance = linker.instantiate(&mut store, &component)?;
    println!("✅ Component instantiated\n");

    // Test the scrape_website function
    println!("🧪 Test 1: Scrape Website with Complex Configuration");

    let scrape_website = exports_exports::get_scrape_website(&instance, &mut store)?;

    let target = ScrapeTarget {
        url: "https://example-shop.com".to_string(),
        selectors: vec![
            Selector::Class("product".to_string()),
            Selector::Tag("h2".to_string()),
            Selector::Class("price".to_string()),
        ],
        required_fields: vec!["name".to_string(), "price".to_string()],
        follow_links: true,
        max_depth: 2,
        delay_ms: Some(100),
    };

    let custom_headers = vec![
        HttpHeader {
            name: "User-Agent".to_string(),
            value: "WebScraperBot/2.0".to_string(),
        },
        HttpHeader {
            name: "Accept-Language".to_string(),
            value: "en-US,en;q=0.9".to_string(),
        },
    ];

    println!("📤 Calling scrape_website with:");
    println!("   URL: {}", target.url);
    println!("   Selectors: {} total", target.selectors.len());
    println!("   Custom Headers: {}", custom_headers.len());
    println!();

    let result = scrape_website.call(&mut store, (target.clone(), None, custom_headers))?;

    match result {
        Ok(scraping_result) => {
            println!("✅ Scraping succeeded!\n");
            println!("📊 Scraping Result:");
            println!("   Target URL: {}", scraping_result.target.url);
            println!("   Data Extracted: {} items", scraping_result.data.len());
            println!("   Errors: {}", scraping_result.errors.len());
            println!();

            // Show some extracted data
            for (idx, data) in scraping_result.data.iter().take(3).enumerate() {
                println!("   📋 Data Item {}:", idx + 1);
                println!("      Field: {}", data.field_name);
                println!("      Value: {:?}", data.value);
                println!("      Source: {}", data.source_url);
                println!("      Confidence: {:.2}", data.confidence);
                println!();
            }

            println!("   ⏱️  Metadata:");
            println!(
                "      Duration: {} ms",
                scraping_result.metadata.duration_ms
            );
            println!(
                "      Pages Visited: {}",
                scraping_result.metadata.pages_visited
            );
            println!(
                "      Elements Extracted: {}",
                scraping_result.metadata.elements_extracted
            );
            println!("      Cache Hits: {}", scraping_result.metadata.cache_hits);
            println!("      Retries: {}", scraping_result.metadata.retry_count);
        }
        Err(error) => {
            println!("❌ Scraping failed: {}", error);
        }
    }

    Ok(())
}
