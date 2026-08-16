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
      { identityId: 101, externalId: '101', displayText: '@John Doe', startUtf16: 9, lengthUtf16: 9 }
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
      { identityId: 999, externalId: '999', displayText: '@Hacker', startUtf16: 30, lengthUtf16: 7 }
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

  it('converts text emoticons to unicode emojis in message text', () => {
    const { getByTestId } = render(MessageMentionText, {
      props: {
        text: "Hok lẽ lôi gemini ra xài luôn trời :') :)",
        mentions: []
      }
    });

    const root = getByTestId('message-mention-text');
    expect(root.textContent).toBe('Hok lẽ lôi gemini ra xài luôn trời 🥲 😊');
  });

  it('uses fallback regex to highlight @mention when mentions prop is empty', () => {
    const { getAllByTestId } = render(MessageMentionText, {
      props: {
        text: '@Hoàng Thọ Nghĩa biến web à xếp, k có toolcall á',
        mentions: []
      }
    });

    const mentionSpans = getAllByTestId('mention-span');
    expect(mentionSpans).toHaveLength(1);
    expect(mentionSpans[0].textContent).toBe('@Hoàng Thọ Nghĩa');
  });

  it('renders HTTP/HTTPS URLs as clickable anchor links', () => {
    const { container } = render(MessageMentionText, {
      props: {
        text: 'Xem thêm tại https://zalo.me/g/vibe-coding và https://google.com nhé',
        mentions: []
      }
    });

    const links = container.querySelectorAll('a');
    expect(links).toHaveLength(2);
    expect(links[0].getAttribute('href')).toBe('https://zalo.me/g/vibe-coding');
    expect(links[1].getAttribute('href')).toBe('https://google.com');
  });

  it('calls onMentionClick with externalId when present', async () => {
    let clickedIdentityId: unknown = undefined;
    let clickedText: unknown = '';
    let clickedExternalId: unknown = undefined;

    const { getAllByTestId } = render(MessageMentionText, {
      props: {
        text: 'Chào @Nct nhé',
        mentions: [
          { identityId: 6045, externalId: '0987654321', displayText: '@Nct', startUtf16: 5, lengthUtf16: 4 }
        ],
        onMentionClick: (identityId, mentionText, externalId) => {
          clickedIdentityId = identityId;
          clickedText = mentionText;
          clickedExternalId = externalId;
        }
      }
    });

    const mentionSpans = getAllByTestId('mention-span');
    expect(mentionSpans).toHaveLength(1);
    await mentionSpans[0].click();

    expect(clickedIdentityId).toBe(6045);
    expect(clickedText).toBe('@Nct');
    expect(clickedExternalId).toBe('0987654321');
  });
});
