"""Tests for content parsers."""

import pytest
from returns.result import Success, Failure

from b00t_j0b_py.parsers import GitHubParser, PyPIParser, NPMParser, CratesParser
from b00t_j0b_py.parsers.base import ParserRegistry


class TestGitHubParser:
    """Test GitHub parser."""
    
    @pytest.fixture
    def parser(self):
        """Create GitHub parser instance."""
        return GitHubParser()
    
    def test_can_parse_github_urls(self, parser):
        """Test GitHub URL detection."""
        github_urls = [
            "https://github.com/user/repo",
            "https://www.github.com/user/repo",
            "https://github.com/user/repo/issues/123",
        ]
        
        for url in github_urls:
            assert parser.can_parse(url)
    
    def test_cannot_parse_non_github_urls(self, parser):
        """Test non-GitHub URL rejection."""
        non_github_urls = [
            "https://pypi.org/project/test",
            "https://example.com",
            "https://npmjs.com/package/test",
        ]
        
        for url in non_github_urls:
            assert not parser.can_parse(url)
    
    def test_parse_repository_page(self, parser):
        """Test parsing GitHub repository page."""
        html_content = """
        <html>
            <head><title>user/repo: Test Repository</title></head>
            <body>
                <h1>user/repo</h1>
                <p data-pjax-container-id="repo-content-pjax-container">Test repository description</p>
                <div data-target="readme-toc.content">
                    <h1>README</h1>
                    <p>This is the README content.</p>
                </div>
                <a class="topic-tag-link">python</a>
                <a class="topic-tag-link">web</a>
            </body>
        </html>
        """
        
        result = parser.parse("https://github.com/user/repo", html_content, "text/html")
        
        assert isinstance(result, Success)
        data = result.unwrap()
        assert "user/repo" in data.title
        assert "Test repository description" in data.content
        assert "README" in data.content
        assert data.metadata["owner"] == "user"
        assert data.metadata["repository"] == "repo"
        assert "python" in data.tags or "web" in data.tags
    
    def test_parse_issue_page(self, parser):
        """Test parsing GitHub issue page."""
        html_content = """
        <html>
            <body>
                <h1><bdi class="js-issue-title">Test Issue Title</bdi></h1>
                <span class="gh-header-number">#123</span>
                <td class="d-block comment-body markdown-body">
                    <p>This is the issue description.</p>
                </td>
                <a class="label-link">bug</a>
                <a class="label-link">help wanted</a>
            </body>
        </html>
        """
        
        result = parser.parse("https://github.com/user/repo/issues/123", html_content, "text/html")
        
        assert isinstance(result, Success)
        data = result.unwrap()
        assert "Test Issue Title" in data.title
        assert "#123" in data.content
        assert "issue description" in data.content
        assert data.metadata["owner"] == "user"
        assert data.metadata["repository"] == "repo"


class TestPyPIParser:
    """Test PyPI parser."""
    
    @pytest.fixture
    def parser(self):
        """Create PyPI parser instance."""
        return PyPIParser()
    
    def test_can_parse_pypi_urls(self, parser):
        """Test PyPI URL detection."""
        pypi_urls = [
            "https://pypi.org/project/test",
            "https://www.pypi.org/project/django/",
            "https://pypi.python.org/project/requests",
        ]
        
        for url in pypi_urls:
            assert parser.can_parse(url)
    
    def test_parse_package_page(self, parser):
        """Test parsing PyPI package page."""
        html_content = """
        <html>
            <body>
                <h1 class="package-header__name">test-package <span>1.2.3</span></h1>
                <p class="package-description__summary">A test package for testing</p>
                <div class="project-description">
                    <h1>Test Package</h1>
                    <p>This is the package description.</p>
                </div>
                <span class="sidebar-section__maintainer">testuser</span>
                <a data-package-name="test-package" href="https://github.com/user/repo">Homepage</a>
            </body>
        </html>
        """
        
        result = parser.parse("https://pypi.org/project/test-package", html_content, "text/html")
        
        assert isinstance(result, Success)
        data = result.unwrap()
        assert "test-package" in data.title
        assert "1.2.3" in data.title
        assert "pip install test-package" in data.content
        assert data.metadata["package_name"] == "test-package"
        assert data.metadata["version"] == "1.2.3"
        assert "testuser" in data.metadata["maintainers"]


class TestNPMParser:
    """Test NPM parser."""
    
    @pytest.fixture
    def parser(self):
        """Create NPM parser instance."""
        return NPMParser()
    
    def test_can_parse_npm_urls(self, parser):
        """Test NPM URL detection."""
        npm_urls = [
            "https://npmjs.com/package/test",
            "https://www.npmjs.com/package/react",
            "https://npmjs.org/package/express/",
        ]
        
        for url in npm_urls:
            assert parser.can_parse(url)
    
    def test_parse_package_page(self, parser):
        """Test parsing NPM package page."""
        html_content = """
        <html>
            <body>
                <h1>test-package</h1>
                <span data-testid="version">2.1.0</span>
                <p data-testid="description">A test NPM package</p>
                <div data-testid="readme">
                    <h1>Test Package</h1>
                    <p>This is the README.</p>
                </div>
                <a href="/search?q=keywords:javascript">javascript</a>
                <a href="/search?q=keywords:testing">testing</a>
            </body>
        </html>
        """
        
        result = parser.parse("https://npmjs.com/package/test-package", html_content, "text/html")
        
        assert isinstance(result, Success)
        data = result.unwrap()
        assert "test-package" in data.title
        assert "npm install test-package" in data.content
        assert data.metadata["package_name"] == "test-package"
        assert "javascript" in data.tags or "testing" in data.tags


class TestCratesParser:
    """Test Crates.io parser stub."""
    
    @pytest.fixture
    def parser(self):
        """Create Crates parser instance."""
        return CratesParser()
    
    def test_can_parse_crates_urls(self, parser):
        """Test crates.io URL detection."""
        crates_urls = [
            "https://crates.io/crates/serde",
            "https://www.crates.io/crates/tokio",
        ]
        
        for url in crates_urls:
            assert parser.can_parse(url)
    
    def test_parse_returns_stub_content(self, parser):
        """Test that parser returns stub content."""
        result = parser.parse("https://crates.io/crates/serde", "<html></html>", "text/html")
        
        assert isinstance(result, Success)
        data = result.unwrap()
        assert "delegated to b00t-c0re-lib" in data.content
        assert data.metadata["delegated_to_rust"] == True
        assert data.metadata["crate_name"] == "serde"


class TestParserRegistry:
    """Test parser registry."""
    
    @pytest.fixture
    def registry(self):
        """Create parser registry."""
        registry = ParserRegistry()
        registry.register(GitHubParser())
        registry.register(PyPIParser())
        registry.register(NPMParser())
        registry.register(CratesParser())
        return registry
    
    def test_get_parser_for_github(self, registry):
        """Test getting parser for GitHub URL."""
        parser = registry.get_parser("https://github.com/user/repo")
        assert isinstance(parser, GitHubParser)
    
    def test_get_parser_for_pypi(self, registry):
        """Test getting parser for PyPI URL."""
        parser = registry.get_parser("https://pypi.org/project/test")
        assert isinstance(parser, PyPIParser)
    
    def test_get_parser_for_npm(self, registry):
        """Test getting parser for NPM URL."""
        parser = registry.get_parser("https://npmjs.com/package/test")
        assert isinstance(parser, NPMParser)
    
    def test_get_parser_for_crates(self, registry):
        """Test getting parser for crates.io URL."""
        parser = registry.get_parser("https://crates.io/crates/serde")
        assert isinstance(parser, CratesParser)
    
    def test_get_parser_for_unknown_url(self, registry):
        """Test getting parser for unknown URL."""
        parser = registry.get_parser("https://unknown.com/page")
        assert parser is None
    
    def test_parse_content_with_registry(self, registry):
        """Test parsing content through registry."""
        html_content = """
        <html>
            <body>
                <h1>user/repo</h1>
                <p>GitHub repository</p>
            </body>
        </html>
        """
        
        result = registry.parse_content("https://github.com/user/repo", html_content, "text/html")
        
        assert isinstance(result, Success)
        data = result.unwrap()
        assert "user/repo" in data.title
    
    def test_parse_content_no_parser(self, registry):
        """Test parsing with no available parser."""
        result = registry.parse_content("https://unknown.com", "<html></html>", "text/html")
        
        assert isinstance(result, Failure)
        assert "No parser available" in str(result.failure())