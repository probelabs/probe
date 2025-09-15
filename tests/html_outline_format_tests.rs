use anyhow::Result;
use std::fs;
use tempfile::TempDir;

mod common;
use common::TestContext;

#[test]
fn test_html_outline_basic_structure() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("basic.html");

    let content = r####"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Test Page</title>
</head>
<body>
    <header class="main-header">
        <nav id="main-nav">
            <ul>
                <li><a href="#home">Home</a></li>
                <li><a href="#about">About</a></li>
            </ul>
        </nav>
        <h1>Welcome to Test Page</h1>
    </header>

    <main class="content">
        <section id="intro">
            <h2>Introduction</h2>
            <p>This is the introduction section.</p>
        </section>

        <section id="features">
            <h2>Features</h2>
            <article class="feature">
                <h3>Feature One</h3>
                <p>Description of feature one.</p>
            </article>
            <article class="feature">
                <h3>Feature Two</h3>
                <p>Description of feature two.</p>
            </article>
        </section>
    </main>

    <aside class="sidebar">
        <h2>Related Links</h2>
        <ul>
            <li><a href="#link1">Link 1</a></li>
            <li><a href="#link2">Link 2</a></li>
        </ul>
    </aside>

    <footer>
        <p>&copy; 2024 Test Company</p>
    </footer>
</body>
</html>"####;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "Welcome",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Check that outline format includes HTML structure
    assert!(output.contains("<h1>"));
    assert!(output.contains("<header"));
    assert!(output.contains("<main"));
    assert!(output.contains("<section"));

    Ok(())
}

#[test]
fn test_html_outline_semantic_elements() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("semantic.html");

    let content = r####"<!DOCTYPE html>
<html>
<head>
    <title>Semantic HTML Test</title>
</head>
<body>
    <header>
        <h1>Main Title</h1>
        <nav>
            <a href="#section1">Section 1</a>
            <a href="#section2">Section 2</a>
        </nav>
    </header>

    <main>
        <section id="section1">
            <h2>Section One</h2>
            <article>
                <h3>Article Title</h3>
                <p>Article content here.</p>
                <figure>
                    <img src="image.jpg" alt="Test image" />
                    <figcaption>Image caption</figcaption>
                </figure>
            </article>
        </section>

        <section id="section2">
            <h2>Section Two</h2>
            <details>
                <summary>More Information</summary>
                <p>Additional details here.</p>
            </details>
        </section>
    </main>

    <aside>
        <h2>Sidebar</h2>
        <p>Sidebar content</p>
    </aside>

    <footer>
        <address>
            Contact: <a href="mailto:test@example.com">test@example.com</a>
        </address>
    </footer>
</body>
</html>"####;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "Section",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Check that semantic elements are properly formatted
    assert!(output.contains("<section"));
    assert!(output.contains("<article>"));
    assert!(output.contains("<figure>"));
    assert!(output.contains("<details>"));
    assert!(output.contains("<aside>"));

    Ok(())
}

#[test]
fn test_html_outline_with_attributes() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("attributes.html");

    let content = r####"<!DOCTYPE html>
<html>
<head>
    <title>HTML with Attributes</title>
</head>
<body>
    <div class="container" id="main-container" data-testid="main-content">
        <h1 class="page-title" id="title">Page Title</h1>

        <form class="contact-form" method="post" action="/submit">
            <fieldset>
                <legend>Contact Information</legend>
                <label for="name">Name:</label>
                <input type="text" id="name" name="name" required />

                <label for="email">Email:</label>
                <input type="email" id="email" name="email" required />

                <button type="submit" class="btn btn-primary">Submit</button>
            </fieldset>
        </form>

        <table class="data-table" id="results-table">
            <thead>
                <tr>
                    <th>Name</th>
                    <th>Email</th>
                    <th>Actions</th>
                </tr>
            </thead>
            <tbody>
                <tr data-row-id="1">
                    <td>John Doe</td>
                    <td>john@example.com</td>
                    <td><button class="btn-edit">Edit</button></td>
                </tr>
            </tbody>
        </table>
    </div>
</body>
</html>"####;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "container",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Check that important attributes are included in signatures
    assert!(output.contains("class=") || output.contains("id="));
    assert!(output.contains("<div"));
    assert!(output.contains("<form"));
    assert!(output.contains("<table"));

    Ok(())
}

#[test]
fn test_html_outline_test_detection() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("test_page.html");

    let content = r####"<!DOCTYPE html>
<html>
<head>
    <title>Test Page</title>
</head>
<body>
    <header>
        <h1>Testing Guide</h1>
    </header>

    <main>
        <section data-testid="test-section" class="test-container">
            <h2>Test Examples</h2>
            <div data-test="example-1">
                <p>This is a test example.</p>
            </div>
        </section>

        <section>
            <h2>Demo Section</h2>
            <p>This section demonstrates features.</p>
        </section>
    </main>

    <script type="text/javascript">
        // Test suite configuration
        describe('Component tests', function() {
            it('should render correctly', function() {
                // Test implementation
            });
        });
    </script>

    <style>
        /* Test-specific styles */
        .test-container {
            border: 1px solid red;
        }
    </style>
</body>
</html>"####;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "test",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Test nodes should be detected and shown
    assert!(output.contains("test") || output.contains("Test"));
    assert!(output.contains("<script"));
    assert!(output.contains("<style"));

    Ok(())
}

#[test]
fn test_html_outline_void_elements() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("void_elements.html");

    let content = r####"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <link rel="stylesheet" href="style.css" />
    <title>Void Elements Test</title>
</head>
<body>
    <h1>Image Gallery</h1>

    <div class="gallery">
        <img src="image1.jpg" alt="Image 1" width="300" height="200" />
        <br />
        <img src="image2.jpg" alt="Image 2" width="300" height="200" />
        <hr />

        <form>
            <input type="text" placeholder="Search images" />
            <input type="submit" value="Search" />
        </form>
    </div>

    <footer>
        <p>Gallery footer content</p>
    </footer>
</body>
</html>"####;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "gallery",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Void elements should be properly formatted with self-closing syntax
    assert!(output.contains("<img") || output.contains("<meta") || output.contains("<input"));

    Ok(())
}

#[test]
fn test_html_outline_comments_and_doctype() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("comments.html");

    let content = r####"<!DOCTYPE html>
<!-- Main HTML document for testing comments -->
<html lang="en">
<head>
    <!-- Meta information -->
    <meta charset="UTF-8">
    <title>Comments Test</title>
</head>
<body>
    <!-- Header section -->
    <header>
        <h1>Page with Comments</h1>
    </header>

    <!-- Main content area -->
    <main>
        <section>
            <!-- TODO: Add more content here -->
            <h2>Content Section</h2>
            <p>This page demonstrates comment handling.</p>
        </section>
    </main>

    <!-- Footer section -->
    <footer>
        <p>Footer content</p>
    </footer>
</body>
</html>"####;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "comment",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Comments and doctype should be included in outline
    assert!(output.contains("<!DOCTYPE") || output.contains("<!--"));

    Ok(())
}

#[test]
fn test_html_outline_complex_structure() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("complex.html");

    let content = r####"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Complex HTML Structure</title>
    <style>
        body { margin: 0; padding: 0; }
        .container { max-width: 1200px; margin: 0 auto; }
    </style>
</head>
<body>
    <div class="wrapper">
        <header class="site-header" role="banner">
            <div class="container">
                <h1 class="site-title">Complex Website</h1>
                <nav class="main-navigation" role="navigation">
                    <ul class="nav-menu">
                        <li><a href="#home">Home</a></li>
                        <li><a href="#about">About</a></li>
                        <li><a href="#services">Services</a></li>
                        <li><a href="#contact">Contact</a></li>
                    </ul>
                </nav>
            </div>
        </header>

        <main class="site-main" role="main">
            <div class="container">
                <section class="hero-section" id="hero">
                    <h2>Welcome to Our Site</h2>
                    <p class="hero-text">This is a complex HTML structure for testing.</p>
                    <button class="cta-button" data-action="scroll-to-content">Get Started</button>
                </section>

                <section class="services-section" id="services">
                    <h2>Our Services</h2>
                    <div class="services-grid">
                        <article class="service-item" data-service="web-design">
                            <h3>Web Design</h3>
                            <p>Professional web design services.</p>
                            <ul class="service-features">
                                <li>Responsive Design</li>
                                <li>Modern Layouts</li>
                                <li>SEO Optimized</li>
                            </ul>
                        </article>

                        <article class="service-item" data-service="development">
                            <h3>Development</h3>
                            <p>Full-stack development solutions.</p>
                            <ul class="service-features">
                                <li>Frontend Development</li>
                                <li>Backend APIs</li>
                                <li>Database Design</li>
                            </ul>
                        </article>
                    </div>
                </section>

                <section class="contact-section" id="contact">
                    <h2>Contact Us</h2>
                    <form class="contact-form" method="post" action="/contact">
                        <fieldset>
                            <legend>Contact Information</legend>
                            <div class="form-group">
                                <label for="contact-name">Name</label>
                                <input type="text" id="contact-name" name="name" required>
                            </div>
                            <div class="form-group">
                                <label for="contact-email">Email</label>
                                <input type="email" id="contact-email" name="email" required>
                            </div>
                            <div class="form-group">
                                <label for="contact-message">Message</label>
                                <textarea id="contact-message" name="message" rows="5" required></textarea>
                            </div>
                            <button type="submit" class="submit-button">Send Message</button>
                        </fieldset>
                    </form>
                </section>
            </div>
        </main>

        <aside class="sidebar" role="complementary">
            <div class="container">
                <section class="widget">
                    <h3>Recent Posts</h3>
                    <ul class="post-list">
                        <li><a href="#post1">How to Build Better Websites</a></li>
                        <li><a href="#post2">Modern CSS Techniques</a></li>
                        <li><a href="#post3">JavaScript Best Practices</a></li>
                    </ul>
                </section>

                <section class="widget">
                    <h3>Newsletter</h3>
                    <form class="newsletter-form">
                        <input type="email" placeholder="Your email" required>
                        <button type="submit">Subscribe</button>
                    </form>
                </section>
            </div>
        </aside>

        <footer class="site-footer" role="contentinfo">
            <div class="container">
                <div class="footer-content">
                    <div class="footer-section">
                        <h4>About Us</h4>
                        <p>We create amazing web experiences.</p>
                    </div>
                    <div class="footer-section">
                        <h4>Quick Links</h4>
                        <ul>
                            <li><a href="#privacy">Privacy Policy</a></li>
                            <li><a href="#terms">Terms of Service</a></li>
                            <li><a href="#support">Support</a></li>
                        </ul>
                    </div>
                    <div class="footer-section">
                        <h4>Follow Us</h4>
                        <div class="social-links">
                            <a href="#" aria-label="Twitter">Twitter</a>
                            <a href="#" aria-label="Facebook">Facebook</a>
                            <a href="#" aria-label="LinkedIn">LinkedIn</a>
                        </div>
                    </div>
                </div>
                <div class="footer-bottom">
                    <p>&copy; 2024 Complex Website. All rights reserved.</p>
                </div>
            </div>
        </footer>
    </div>

    <script>
        // Simple interaction script
        document.addEventListener('DOMContentLoaded', function() {
            const ctaButton = document.querySelector('.cta-button');
            if (ctaButton) {
                ctaButton.addEventListener('click', function() {
                    const servicesSection = document.getElementById('services');
                    if (servicesSection) {
                        servicesSection.scrollIntoView({ behavior: 'smooth' });
                    }
                });
            }
        });
    </script>
</body>
</html>"####;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "services",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Complex structure should be properly outlined
    assert!(output.contains("<section"));
    assert!(output.contains("<article"));
    assert!(output.contains("<header"));
    assert!(output.contains("<main"));
    assert!(output.contains("<aside"));
    assert!(output.contains("<footer"));

    Ok(())
}

#[test]
fn test_html_language_detection() {
    use probe_code::language::factory::get_language_impl;

    // Test .html extension
    let lang_impl = get_language_impl("html");
    assert!(lang_impl.is_some());

    // Test .htm extension
    let lang_impl = get_language_impl("htm");
    assert!(lang_impl.is_some());
}

#[test]
fn test_html_with_various_extensions() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Test both .html and .htm extensions
    let extensions = vec!["html", "htm"];

    for ext in extensions {
        let file_path = temp_dir.path().join(format!("test.{}", ext));
        let content = r####"<!DOCTYPE html>
<html>
<head>
    <title>Test Document</title>
</head>
<body>
    <header>
        <h1>Test Page</h1>
    </header>
    <main>
        <section>
            <h2>Content Section</h2>
            <p>This is a test page with HTML content.</p>
        </section>
    </main>
    <footer>
        <p>Footer content</p>
    </footer>
</body>
</html>"####;

        fs::write(&file_path, content)?;

        let ctx = TestContext::new();
        let output = ctx.run_probe(&[
            "search",
            "Content",
            file_path.to_str().unwrap(),
            "--format",
            "outline",
        ])?;

        // Should find content and show proper HTML structure
        assert!(!output.is_empty(), "No output found for .{} file", ext);
        assert!(
            output.contains("Content") || output.contains("<section") || output.contains("<h2")
        );
    }

    Ok(())
}
