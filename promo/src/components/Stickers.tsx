import React from 'react';
import { Img, staticFile } from 'remotion';
import { Gif } from '@remotion/gif';

/**
 * Inline stickers / images, shown the way NanaType does for user-supplied files.
 * kind: 'cat' → animated GIF sticker, 'img' → a local picture (meme.jpg).
 * Assets live in public/media (git-ignored) and were supplied by the project owner.
 */
export const Sticker: React.FC<{ kind: string; size: number }> = ({ kind, size }) => {
  if (kind === 'img') {
    return (
      <Img
        src={staticFile('media/meme.jpg')}
        style={{
          height: size, width: size * (340 / 322), objectFit: 'cover', borderRadius: size * 0.1,
          verticalAlign: 'middle', boxShadow: '0 3px 10px #1b2f4b44',
        }}
      />
    );
  }
  return (
    <span style={{ display: 'inline-block', width: size, height: size, verticalAlign: 'middle' }}>
      <Gif src={staticFile('media/sticker.gif')} width={size} height={size} fit="contain" />
    </span>
  );
};
