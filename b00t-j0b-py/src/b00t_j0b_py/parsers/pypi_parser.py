"""PyPI-specific content parser."""

import json
import re
from typing import Dict, Any, List
from urllib.parse import urlparse
from bs4 import BeautifulSoup
from returns.result import Result, Success, Failure
import markdownify

from .base import BaseParser, ParseResult


class PyPIParser(BaseParser):
    """Parser for PyPI package pages."""
    
    def can_parse(self, url: str) -> bool:
        """Check if URL is from PyPI."""
        domain = self.get_domain(url)
        return domain in ["pypi.org", "www.pypi.org", "pypi.python.org"]
    
    def parse(self, url: str, content: str, content_type: str) -> Result[ParseResult, Exception]:
        """Parse PyPI package content."""
        try:
            path_segments = self.get_path_segments(url)
            
            if len(path_segments) >= 2 and path_segments[0] == "project":
                package_name = path_segments[1]
                return self._parse_package(url, content, package_name)
            
            # Fallback to generic parsing
            return self._parse_generic(url, content)
            
        except Exception as e:
            return Failure(e)
    
    def _parse_package(self, url: str, content: str, package_name: str) -> Result[ParseResult, Exception]:
        """Parse PyPI package page."""
        try:
            soup = BeautifulSoup(content, 'html.parser')
            
            # Extract package title and version
            title = package_name
            title_elem = soup.find('h1', class_='package-header__name')
            if title_elem:
                title = title_elem.get_text().strip()
            
            # Extract version
            version = ""
            version_elem = soup.find('h1', class_='package-header__name')
            if version_elem:
                version_span = version_elem.find('span')
                if version_span:
                    version = version_span.get_text().strip()
            
            # Extract description
            description = ""
            desc_elem = soup.find('p', class_='package-description__summary')
            if desc_elem:
                description = desc_elem.get_text().strip()
            
            # Extract project URLs
            project_urls = {}
            url_elems = soup.find_all('a', {'data-package-name': package_name})
            for url_elem in url_elems:
                href = url_elem.get('href', '')
                text = url_elem.get_text().strip()
                if href and text:
                    project_urls[text] = href
            
            # Extract installation command
            install_cmd = f"pip install {package_name}"
            if version:
                install_cmd = f"pip install {package_name}=={version}"
            
            # Extract readme/description content
            readme_content = ""
            readme_elem = soup.find('div', class_='project-description')
            if readme_elem:
                readme_content = markdownify.markdownify(str(readme_elem))
            
            # Extract classifiers/categories
            classifiers = []
            classifier_elems = soup.find_all('p', string=re.compile('Classifier:'))
            for elem in classifier_elems:
                classifier_text = elem.get_text().replace('Classifier:', '').strip()
                if classifier_text:
                    classifiers.append(classifier_text)
            
            # Extract maintainers
            maintainers = []
            maintainer_elems = soup.find_all('span', class_='sidebar-section__maintainer')
            for elem in maintainer_elems:
                maintainer = elem.get_text().strip()
                if maintainer:
                    maintainers.append(maintainer)
            
            # Extract tags from classifiers
            tags = []
            for classifier in classifiers:
                if '::' in classifier:
                    parts = classifier.split('::')
                    tags.extend(part.strip() for part in parts if part.strip())
            
            # Remove duplicates and clean tags
            tags = list(set(tag for tag in tags if len(tag) > 1))
            
            # Build markdown content
            markdown_parts = [
                f"# {title}",
                f"\n**Package:** {package_name}",
                f"**Version:** {version}" if version else "",
                f"**URL:** {url}",
                f"\n**Installation:**\n```bash\n{install_cmd}\n```\n"
            ]
            
            if description:
                markdown_parts.append(f"**Description:** {description}\n")
            
            if maintainers:
                markdown_parts.append(f"**Maintainers:** {', '.join(maintainers)}\n")
            
            if project_urls:
                markdown_parts.append("**Project URLs:**")
                for name, link in project_urls.items():
                    markdown_parts.append(f"- [{name}]({link})")
                markdown_parts.append("")
            
            if classifiers:
                markdown_parts.append("**Classifiers:**")
                for classifier in classifiers[:10]:  # Limit to first 10
                    markdown_parts.append(f"- {classifier}")
                markdown_parts.append("")
            
            if readme_content:
                markdown_parts.append("## Project Description\n")
                markdown_parts.append(readme_content)
            
            markdown_content = "\n".join(filter(None, markdown_parts))
            
            return Success(ParseResult(
                url=url,
                title=f"{package_name} ({version})" if version else package_name,
                content=markdown_content,
                content_type="text/markdown",
                metadata={
                    "package_name": package_name,
                    "version": version,
                    "description": description,
                    "maintainers": maintainers,
                    "project_urls": project_urls,
                    "classifiers": classifiers,
                    "install_command": install_cmd
                },
                tags=tags[:20]  # Limit tags
            ))
            
        except Exception as e:
            return Failure(e)
    
    def _parse_generic(self, url: str, content: str) -> Result[ParseResult, Exception]:
        """Generic PyPI page parsing."""
        try:
            soup = BeautifulSoup(content, 'html.parser')
            
            title = "PyPI Page"
            title_elem = soup.find('title')
            if title_elem:
                title = title_elem.get_text().strip()
            
            # Convert to markdown
            markdown_content = markdownify.markdownify(content)
            
            return Success(ParseResult(
                url=url,
                title=title,
                content=markdown_content,
                content_type="text/markdown",
                metadata={"platform": "pypi"}
            ))
            
        except Exception as e:
            return Failure(e)