import { describe, it, expect } from 'vitest';
import { removeFirstToken } from '../src/utils';

describe('removeFirstToken', () => {
  it('removes a token from the middle of a query', () => {
    expect(removeFirstToken('invoices pdf 2024', 'pdf')).toBe('invoices 2024');
  });

  it('removes a token from the start', () => {
    expect(removeFirstToken('pdf invoices', 'pdf')).toBe('invoices');
  });

  it('removes a token from the end', () => {
    expect(removeFirstToken('invoices pdf', 'pdf')).toBe('invoices');
  });

  it('removes only the first occurrence when the token appears twice', () => {
    expect(removeFirstToken('pdf invoices pdf', 'pdf')).toBe('invoices pdf');
  });

  it('does not remove a token that appears as a substring of another word', () => {
    expect(removeFirstToken('pdfviewer invoices', 'pdf')).toBe('pdfviewer invoices');
  });

  it('is case-insensitive', () => {
    expect(removeFirstToken('invoices PDF 2024', 'pdf')).toBe('invoices 2024');
  });

  it('collapses multiple spaces left by removal', () => {
    expect(removeFirstToken('invoices  pdf  2024', 'pdf')).toBe('invoices 2024');
  });

  it('returns empty string when the query is just the token', () => {
    expect(removeFirstToken('pdf', 'pdf')).toBe('');
  });

  it('returns the query unchanged when the token is not present', () => {
    expect(removeFirstToken('invoices docx 2024', 'pdf')).toBe('invoices docx 2024');
  });
});
