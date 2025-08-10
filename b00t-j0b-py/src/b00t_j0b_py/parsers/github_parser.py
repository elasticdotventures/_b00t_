"""GitHub-specific content parser."""

import json
import re
from typing import Dict, Any, List, Optional
from urllib.parse import urlparse
from bs4 import BeautifulSoup
from returns.result import Result, Success, Failure
import markdownify

from .base import BaseParser, ParseResult


class GitHubParser(BaseParser):
    """Parser for GitHub repositories, issues, PRs, etc."""
    
    def can_parse(self, url: str) -> bool:
        """Check if URL is from GitHub."""
        domain = self.get_domain(url)
        return domain in ["github.com", "www.github.com"]
    
    def parse(self, url: str, content: str, content_type: str) -> Result[ParseResult, Exception]:
        """Parse GitHub content."""
        try:
            path_segments = self.get_path_segments(url)
            
            if len(path_segments) >= 2:
                owner, repo = path_segments[0], path_segments[1]
                
                # Determine content type by URL pattern
                if len(path_segments) == 2:
                    # Repository main page
                    return self._parse_repository(url, content, owner, repo)
                elif len(path_segments) >= 3:
                    section = path_segments[2]
                    if section == "issues":
                        return self._parse_issue(url, content, owner, repo)
                    elif section == "pull":
                        return self._parse_pull_request(url, content, owner, repo)
                    elif section == "blob":
                        return self._parse_file_blob(url, content, owner, repo, path_segments[3:])
                    elif section == "tree":
                        return self._parse_directory(url, content, owner, repo)
                    elif section == "releases":
                        return self._parse_releases(url, content, owner, repo)
                    elif section == "wiki":
                        return self._parse_wiki(url, content, owner, repo)
            
            # Fallback to generic parsing
            return self._parse_generic(url, content)
            
        except Exception as e:
            return Failure(e)
    
    def _parse_repository(self, url: str, content: str, owner: str, repo: str) -> Result[ParseResult, Exception]:
        """Parse main repository page."""
        try:
            soup = BeautifulSoup(content, 'html.parser')
            
            # Extract title
            title = f"{owner}/{repo}"
            title_elem = soup.find('h1')
            if title_elem:
                title = title_elem.get_text().strip()
            
            # Extract description
            description = ""
            desc_elem = soup.find('p', {'data-pjax-container-id': 'repo-content-pjax-container'})
            if not desc_elem:
                desc_elem = soup.find('p', class_=re.compile('.*repo.*description.*'))
            if desc_elem:
                description = desc_elem.get_text().strip()
            
            # Extract README content
            readme_content = ""
            readme_elem = soup.find('div', {'data-target': 'readme-toc.content'})
            if not readme_elem:
                # Try alternative selectors
                readme_elem = soup.find('article', class_=re.compile('.*readme.*'))
            if readme_elem:
                readme_content = markdownify.markdownify(str(readme_elem))
            
            # Extract topics/tags
            tags = []
            topic_elems = soup.find_all('a', class_=re.compile('.*topic-tag.*'))
            for topic_elem in topic_elems:
                tags.append(topic_elem.get_text().strip())
            
            # Extract language info
            languages = []
            lang_elems = soup.find_all('span', {'data-view-component': True})
            for lang_elem in lang_elems:
                if 'language-color' in str(lang_elem):
                    lang_text = lang_elem.find_next_sibling(string=True)
                    if lang_text:
                        languages.append(lang_text.strip())
            
            # Extract stats
            stats = self._extract_repo_stats(soup)
            
            # Build markdown content
            markdown_parts = [
                f"# {title}",
                f"\n**Repository:** [{owner}/{repo}]({url})\n"
            ]
            
            if description:
                markdown_parts.append(f"**Description:** {description}\n")
            
            if languages:
                markdown_parts.append(f"**Languages:** {', '.join(languages)}\n")
            
            if tags:
                markdown_parts.append(f"**Topics:** {', '.join(tags)}\n")
            
            if stats:
                stats_md = "**Stats:**\n"
                for key, value in stats.items():
                    stats_md += f"- {key}: {value}\n"
                markdown_parts.append(stats_md)
            
            if readme_content:
                markdown_parts.append("\n## README\n")
                markdown_parts.append(readme_content)
            
            markdown_content = "\n".join(markdown_parts)
            
            return Success(ParseResult(
                url=url,
                title=title,
                content=markdown_content,
                content_type="text/markdown",
                metadata={
                    "owner": owner,
                    "repository": repo,
                    "description": description,
                    "languages": languages,
                    "stats": stats
                },
                tags=tags
            ))
            
        except Exception as e:
            return Failure(e)
    
    def _parse_issue(self, url: str, content: str, owner: str, repo: str) -> Result[ParseResult, Exception]:
        """Parse GitHub issue."""
        try:
            soup = BeautifulSoup(content, 'html.parser')
            
            # Extract issue title
            title_elem = soup.find('h1', class_=re.compile('.*js-issue-title.*'))
            if not title_elem:
                title_elem = soup.find('bdi', class_='js-issue-title')
            title = title_elem.get_text().strip() if title_elem else "GitHub Issue"
            
            # Extract issue number
            issue_number = ""
            number_elem = soup.find('span', class_='gh-header-number')
            if number_elem:
                issue_number = number_elem.get_text().strip()
            
            # Extract issue body
            body_elem = soup.find('td', class_='d-block comment-body markdown-body')
            body = ""
            if body_elem:
                body = markdownify.markdownify(str(body_elem))
            
            # Extract labels
            labels = []
            label_elems = soup.find_all('a', class_=re.compile('.*label.*'))
            for label_elem in label_elems:
                label_text = label_elem.get_text().strip()
                if label_text and label_text not in labels:
                    labels.append(label_text)
            
            # Build markdown
            markdown_content = f"""# {title}

**Issue:** {issue_number}
**Repository:** [{owner}/{repo}]({url.split('/issues')[0]})
**URL:** {url}

{f"**Labels:** {', '.join(labels)}" if labels else ""}

## Description

{body}
"""
            
            return Success(ParseResult(
                url=url,
                title=f"{title} ({issue_number})",
                content=markdown_content,
                content_type="text/markdown",
                metadata={
                    "owner": owner,
                    "repository": repo,
                    "issue_number": issue_number,
                    "labels": labels
                },
                tags=labels
            ))
            
        except Exception as e:
            return Failure(e)
    
    def _parse_pull_request(self, url: str, content: str, owner: str, repo: str) -> Result[ParseResult, Exception]:
        """Parse GitHub pull request."""
        # Similar to issue parsing but with PR-specific elements
        return self._parse_issue(url, content, owner, repo)  # Simplified for now
    
    def _parse_file_blob(self, url: str, content: str, owner: str, repo: str, path_segments: List[str]) -> Result[ParseResult, Exception]:
        """Parse individual file view."""
        try:
            soup = BeautifulSoup(content, 'html.parser')
            
            # Extract file path
            file_path = "/".join(path_segments[1:]) if len(path_segments) > 1 else ""
            
            # Extract file content
            file_content = ""
            code_elem = soup.find('table', {'data-tagsearch-lang': True})
            if not code_elem:
                code_elem = soup.find('div', class_=re.compile('.*blob-wrapper.*'))
            
            if code_elem:
                # Try to get raw text content
                lines = code_elem.find_all('td', class_='blob-code-inner')
                if lines:
                    file_content = '\n'.join(line.get_text() for line in lines)
                else:
                    file_content = code_elem.get_text()
            
            # Determine file type
            file_ext = file_path.split('.')[-1].lower() if '.' in file_path else ""
            
            markdown_content = f"""# {file_path}

**Repository:** [{owner}/{repo}]({url.split('/blob')[0]})
**File Path:** `{file_path}`
**URL:** {url}

```{file_ext}
{file_content}
```
"""
            
            return Success(ParseResult(
                url=url,
                title=f"{owner}/{repo}: {file_path}",
                content=markdown_content,
                content_type="text/markdown",
                metadata={
                    "owner": owner,
                    "repository": repo,
                    "file_path": file_path,
                    "file_extension": file_ext
                },
                tags=[file_ext] if file_ext else []
            ))
            
        except Exception as e:
            return Failure(e)
    
    def _parse_directory(self, url: str, content: str, owner: str, repo: str) -> Result[ParseResult, Exception]:
        """Parse directory listing."""
        try:
            soup = BeautifulSoup(content, 'html.parser')
            
            # Extract directory path
            path_elem = soup.find('nav', {'aria-label': 'Breadcrumb'})
            dir_path = ""
            if path_elem:
                breadcrumbs = path_elem.find_all('a')
                if len(breadcrumbs) > 2:  # Skip owner/repo
                    dir_path = "/".join(a.get_text().strip() for a in breadcrumbs[2:])
            
            # Extract file listings
            files = []
            file_rows = soup.find_all('div', role='rowheader')
            for row in file_rows:
                link_elem = row.find('a')
                if link_elem:
                    files.append({
                        'name': link_elem.get_text().strip(),
                        'url': link_elem.get('href', '')
                    })
            
            # Build markdown
            markdown_content = f"""# Directory: {dir_path or '/'}

**Repository:** [{owner}/{repo}]({url.split('/tree')[0]})
**Path:** `{dir_path or '/'}`
**URL:** {url}

## Contents

"""
            
            for file_info in files:
                markdown_content += f"- [{file_info['name']}]({file_info['url']})\n"
            
            return Success(ParseResult(
                url=url,
                title=f"{owner}/{repo}: {dir_path or '/'}",
                content=markdown_content,
                content_type="text/markdown",
                metadata={
                    "owner": owner,
                    "repository": repo,
                    "directory_path": dir_path,
                    "files": files
                }
            ))
            
        except Exception as e:
            return Failure(e)
    
    def _parse_releases(self, url: str, content: str, owner: str, repo: str) -> Result[ParseResult, Exception]:
        """Parse releases page."""
        # Placeholder implementation
        return self._parse_generic(url, content)
    
    def _parse_wiki(self, url: str, content: str, owner: str, repo: str) -> Result[ParseResult, Exception]:
        """Parse wiki page."""
        # Placeholder implementation
        return self._parse_generic(url, content)
    
    def _extract_repo_stats(self, soup: BeautifulSoup) -> Dict[str, Any]:
        """Extract repository statistics."""
        stats = {}
        
        try:
            # Look for stats in various locations
            stat_elems = soup.find_all('a', class_=re.compile('.*Link--primary.*'))
            
            for elem in stat_elems:
                text = elem.get_text().strip()
                if 'star' in text.lower():
                    stars = re.findall(r'[\d,]+', text)
                    if stars:
                        stats['stars'] = stars[0]
                elif 'fork' in text.lower():
                    forks = re.findall(r'[\d,]+', text)
                    if forks:
                        stats['forks'] = forks[0]
                elif 'watch' in text.lower():
                    watchers = re.findall(r'[\d,]+', text)
                    if watchers:
                        stats['watchers'] = watchers[0]
        
        except Exception:
            pass
        
        return stats
    
    def _parse_generic(self, url: str, content: str) -> Result[ParseResult, Exception]:
        """Generic GitHub page parsing."""
        try:
            soup = BeautifulSoup(content, 'html.parser')
            
            title = "GitHub Page"
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
                metadata={"platform": "github"}
            ))
            
        except Exception as e:
            return Failure(e)