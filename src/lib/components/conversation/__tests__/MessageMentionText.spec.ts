import { describe, it, expect, afterEach } from 'bun:test';
import { render, cleanup } from '@testing-library/svelte';
import MessageMentionText from '../MessageMentionText.svelte';

describe('MessageMentionText.svelte', () => {
  afterEach(cleanup);
  it('renders plain text when no mentions are provided', () => {
    const { getByTestId, container } = render(MessageMentionText, {
      props: {
        text: 'Hello world'
      }
    });

    const root = getByTestId('message-mention-text');
    expect(root.textContent).toBe('Hello world');
    expect(container.querySelectorAll('[data-testid="mention-span"]')).toHaveLength(0);
  });

  it('renders text with highlighted mention spans', () => {
    const text = 'Xin chào @John Doe chúc một ngày tốt lành!';
    const mentions = [
      { identityId: 101, displayText: '@John Doe', startUtf16: 9, lengthUtf16: 9 }
    ];

    const { getByTestId, getAllByTestId } = render(MessageMentionText, {
      props: {
        text,
        mentions
      }
    });

    const root = getByTestId('message-mention-text');
    expect(root.textContent).toBe('Xin chào @John Doe chúc một ngày tốt lành!');

    const mentionSpans = getAllByTestId('mention-span');
    expect(mentionSpans).toHaveLength(1);
    expect(mentionSpans[0].textContent).toBe('@John Doe');
    expect(mentionSpans[0].getAttribute('data-mention-id')).toBe('101');
  });

  it('prevents HTML injection by rendering raw text safely', () => {
    const text = '<script>alert("xss")</script> @Hacker';
    const mentions = [
      { identityId: 999, displayText: '@Hacker', startUtf16: 30, lengthUtf16: 7 }
    ];

    const { getByTestId, container } = render(MessageMentionText, {
      props: {
        text,
        mentions
      }
    });

    const root = getByTestId('message-mention-text');
    expect(root.textContent).toBe('<script>alert("xss")</script> @Hacker');
    expect(container.querySelector('script')).toBeNull();
  });
});
