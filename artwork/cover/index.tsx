import React from 'react';
import { registerRoot, Still } from 'remotion';
import { Cover, DouyinCover } from './Cover';

registerRoot(() => <>
  <Still id="cover-16x9" component={Cover} width={1920} height={1080} />
  <Still id="cover-4x3" component={Cover} defaultProps={{ cropToFourThree: true }} width={1440} height={1080} />
  <Still id="cover-standard" component={Cover} width={1920} height={1200} />
  <Still id="cover-portrait" component={DouyinCover} width={1080} height={1440} />
</>);
