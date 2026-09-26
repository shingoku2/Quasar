import React, { Suspense } from 'react';
import SshFileManager from '../SshFileManager';

// xterm.js is ~333 kB of the bundle and is only needed once a user actually opens
// an SSH session, so it is split into its own chunk and loaded on demand.
const TerminalComponent = React.lazy(() => import('../TerminalComponent'));

/**
 * An open session tab as data. Its content is rendered from this at render time, so
 * no handler or prop is frozen into tab state (FE-018).
 */
export type SessionDescriptor =
  | { kind: 'ssh' | 'sftp'; id: string; title: string; address: string; port: number; username: string; password?: string; credentialId?: string }
  | { kind: 'rdp'; id: string; title: string; address: string };

const SessionContent: React.FC<{ session: SessionDescriptor }> = ({ session }) => {
  switch (session.kind) {
    case 'ssh':
      return (
        <Suspense fallback={<div className="p-4 text-gray-400 text-sm">Loading terminal…</div>}>
          <TerminalComponent
            sessionId={session.id}
            host={session.address}
            port={session.port}
            username={session.username}
            password={session.password}
            credentialId={session.credentialId}
          />
        </Suspense>
      );
    case 'sftp':
      return (
        <SshFileManager
          host={session.address}
          port={session.port}
          username={session.username}
          password={session.password}
          credentialId={session.credentialId}
        />
      );
    case 'rdp':
      return (
        <div className="p-10 text-center">
          <h2 className="text-xl text-blue-400 mb-2">RDP Session Launched</h2>
          <p className="text-gray-400">Launched RDP client for {session.address}</p>
        </div>
      );
  }
};

export default SessionContent;
