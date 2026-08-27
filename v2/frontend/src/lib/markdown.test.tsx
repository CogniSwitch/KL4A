import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { renderMarkdown } from './markdown';

function renderMd(markdown: string) {
  return render(<>{renderMarkdown(markdown)}</>);
}

describe('renderMarkdown', () => {
  it('renders a plain paragraph', () => {
    const { container } = renderMd('Just a sentence.');
    expect(container.querySelector('p')?.textContent).toBe('Just a sentence.');
  });

  it('renders bold inline text inside a paragraph', () => {
    const { container } = renderMd('Approved by **the reviewer** today.');
    const strong = container.querySelector('strong');
    expect(strong?.textContent).toBe('the reviewer');
    expect(container.querySelector('p')?.textContent).toBe('Approved by the reviewer today.');
  });

  it('renders headings at three levels', () => {
    const { container } = renderMd('## Section\n### Subsection\n#### Detail');
    expect(container.querySelector('h2')?.textContent).toBe('Section');
    expect(container.querySelector('h3')?.textContent).toBe('Subsection');
    expect(container.querySelector('h4')?.textContent).toBe('Detail');
  });

  it('renders a horizontal rule', () => {
    const { container } = renderMd('above\n\n---\n\nbelow');
    expect(container.querySelector('hr')).not.toBeNull();
  });

  it('groups consecutive "- " lines into one bullet list', () => {
    const { container } = renderMd('- first\n- second\n- third');
    const items = container.querySelectorAll('ul > li');
    expect(items).toHaveLength(3);
    expect(items[0].textContent).toBe('first');
    expect(items[2].textContent).toBe('third');
  });

  it('starts a new list after a paragraph breaks the run', () => {
    const { container } = renderMd('- a\n- b\n\nnot a list item\n\n- c');
    const lists = container.querySelectorAll('ul');
    expect(lists).toHaveLength(2);
    expect(lists[0].querySelectorAll('li')).toHaveLength(2);
    expect(lists[1].querySelectorAll('li')).toHaveLength(1);
  });

  it('renders a GFM pipe table with header and body rows', () => {
    const markdown = ['| Rule | Result |', '|---|---|', '| Age >= 65 | Escalate |', '| **Age < 65** | Monitor |'].join('\n');
    const { container } = renderMd(markdown);
    const headers = container.querySelectorAll('table thead th');
    expect(Array.from(headers).map((h) => h.textContent)).toEqual(['Rule', 'Result']);
    const rows = container.querySelectorAll('table tbody tr');
    expect(rows).toHaveLength(2);
    expect(rows[0].querySelectorAll('td')[0].textContent).toBe('Age >= 65');
    expect(rows[1].querySelectorAll('td')[0].querySelector('strong')?.textContent).toBe('Age < 65');
  });

  it('does not misread an ordinary pipe-bounded line as a table without a separator row', () => {
    const { container } = renderMd('|not a table|');
    expect(container.querySelector('table')).toBeNull();
    expect(container.querySelector('p')?.textContent).toBe('|not a table|');
  });
});
