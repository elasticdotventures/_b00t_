"""NPM-specific content parser."""

import json
import re
from typing import Dict, Any, List
from urllib.parse import urlparse
from bs4 import BeautifulSoup
from returns.result import Result, Success, Failure
import markdownify

from .base import BaseParser, ParseResult


class NPMParser(BaseParser):
    """Parser for NPM package pages."""
    
    def can_parse(self, url: str) -> bool:
        """Check if URL is from NPM."""
        domain = self.get_domain(url)
        return domain in ["npmjs.com", "www.npmjs.com", "npmjs.org", "www.npmjs.org"]
    
    def parse(self, url: str, content: str, content_type: str) -> Result[ParseResult, Exception]:
        """Parse NPM package content."""
        try:
            path_segments = self.get_path_segments(url)
            
            if len(path_segments) >= 2 and path_segments[0] == "package":
                package_name = path_segments[1]
                return self._parse_package(url, content, package_name)
            
            # Fallback to generic parsing
            return self._parse_generic(url, content)
            
        except Exception as e:
            return Failure(e)
    
    def _parse_package(self, url: str, content: str, package_name: str) -> Result[ParseResult, Exception]:
        """Parse NPM package page."""
        try:
            soup = BeautifulSoup(content, 'html.parser')
            
            # Extract package title and version
            title = package_name
            title_elem = soup.find('h1')
            if title_elem:
                title = title_elem.get_text().strip()
            
            # Extract version
            version = ""
            version_elem = soup.find('span', {'data-testid': 'version'})
            if not version_elem:
                # Try alternative selectors
                version_elem = soup.find('p', string=re.compile(r'^\d+\.\d+\.\d+'))
            if version_elem:
                version_text = version_elem.get_text().strip()
                version_match = re.search(r'(\d+\.\d+\.\d+[^\s]*)', version_text)
                if version_match:
                    version = version_match.group(1)
            
            # Extract description
            description = ""
            desc_elem = soup.find('p', {'data-testid': 'description'})
            if not desc_elem:
                # Try meta description
                meta_desc = soup.find('meta', {'name': 'description'})
                if meta_desc:
                    description = meta_desc.get('content', '')
            else:
                description = desc_elem.get_text().strip()
            
            # Extract installation command
            install_cmd = f"npm install {package_name}"
            
            # Extract repository URL
            repo_url = ""
            repo_elem = soup.find('a', href=re.compile(r'github\.com'))
            if repo_elem:
                repo_url = repo_elem.get('href', '')
            
            # Extract homepage URL
            homepage_url = ""
            homepage_elem = soup.find('a', {'data-testid': 'homepage'})
            if homepage_elem:
                homepage_url = homepage_elem.get('href', '')
            
            # Extract README content
            readme_content = ""
            readme_elem = soup.find('div', {'data-testid': 'readme'})
            if not readme_elem:
                # Try alternative selector
                readme_elem = soup.find('section', id='readme')
            if readme_elem:
                readme_content = markdownify.markdownify(str(readme_elem))
            
            # Extract keywords/tags
            keywords = []
            keyword_elems = soup.find_all('a', href=re.compile(r'/search\?q=keywords:'))
            for elem in keyword_elems:
                keyword = elem.get_text().strip()
                if keyword:
                    keywords.append(keyword)
            
            # Extract dependencies info
            dependencies = self._extract_dependencies(soup)
            
            # Extract stats
            stats = self._extract_package_stats(soup)
            
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
            
            if repo_url:
                markdown_parts.append(f"**Repository:** {repo_url}\n")
            
            if homepage_url:
                markdown_parts.append(f"**Homepage:** {homepage_url}\n")
            
            if keywords:
                markdown_parts.append(f"**Keywords:** {', '.join(keywords)}\n")
            
            if stats:
                stats_md = "**Stats:**\n"
                for key, value in stats.items():
                    stats_md += f"- {key}: {value}\n"
                markdown_parts.append(stats_md)
            
            if dependencies:
                deps_md = "**Dependencies:**\n"
                for dep_type, deps in dependencies.items():
                    if deps:
                        deps_md += f"- {dep_type}: {len(deps)} packages\n"
                markdown_parts.append(deps_md)
            
            if readme_content:
                markdown_parts.append("## README\n")
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
                    "repository_url": repo_url,
                    "homepage_url": homepage_url,
                    "dependencies": dependencies,
                    "stats": stats,
                    "install_command": install_cmd
                },
                tags=keywords
            ))
            
        except Exception as e:
            return Failure(e)
    
    def _extract_dependencies(self, soup: BeautifulSoup) -> Dict[str, List[str]]:
        """Extract dependency information."""
        dependencies = {
            "dependencies": [],
            "devDependencies": [],
            "peerDependencies": []
        }
        
        try:
            # Look for dependency sections
            dep_sections = soup.find_all('section', {'data-testid': re.compile(r'.*dependencies.*')})
            for section in dep_sections:
                section_title = section.find('h3')
                if section_title:
                    title_text = section_title.get_text().strip().lower()
                    
                    # Extract dependency links
                    dep_links = section.find_all('a', href=re.compile(r'/package/'))
                    deps = [link.get_text().strip() for link in dep_links]
                    
                    if 'dev' in title_text:
                        dependencies["devDependencies"] = deps
                    elif 'peer' in title_text:
                        dependencies["peerDependencies"] = deps
                    else:
                        dependencies["dependencies"] = deps
        
        except Exception:
            pass
        
        return dependencies
    
    def _extract_package_stats(self, soup: BeautifulSoup) -> Dict[str, Any]:
        """Extract package statistics."""
        stats = {}
        
        try:
            # Look for download stats
            download_elem = soup.find('p', string=re.compile(r'weekly downloads'))
            if not download_elem:
                download_elem = soup.find('span', {'data-testid': 'weekly-downloads'})
            
            if download_elem:
                download_text = download_elem.get_text()
                downloads = re.findall(r'[\d,]+', download_text)
                if downloads:
                    stats['weekly_downloads'] = downloads[0]
            
            # Look for version info
            version_elem = soup.find('p', string=re.compile(r'last publish'))
            if version_elem:
                stats['last_publish'] = version_elem.get_text().strip()
            
            # Look for unpacked size
            size_elem = soup.find('p', string=re.compile(r'unpacked size'))
            if size_elem:
                size_text = size_elem.get_text()
                size_match = re.search(r'([\d.]+ [kMG]?B)', size_text)
                if size_match:
                    stats['unpacked_size'] = size_match.group(1)
        
        except Exception:
            pass
        
        return stats
    
    def _parse_generic(self, url: str, content: str) -> Result[ParseResult, Exception]:
        """Generic NPM page parsing."""
        try:
            soup = BeautifulSoup(content, 'html.parser')
            
            title = "NPM Page"
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
                metadata={"platform": "npm"}
            ))
            
        except Exception as e:
            return Failure(e)