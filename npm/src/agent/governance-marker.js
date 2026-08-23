// This module is intentionally not part of the public package surface.  The
// marker gives governed integrations a non-structural identity check without
// making a plain object with the right fields sufficient.
const PROBE_AGENT_AUTHENTICITY = Symbol('ProbeAgentAuthenticity');

export function markProbeAgent(agent) {
  Object.defineProperty(agent, PROBE_AGENT_AUTHENTICITY, {
    value: true,
    enumerable: false,
    configurable: false,
    writable: false
  });
  return agent;
}

export function isAuthenticProbeAgent(agent) {
  return !!agent && agent[PROBE_AGENT_AUTHENTICITY] === true;
}

// A narrowly scoped hook for isolated unit tests that need to exercise the
// real governed server with a small test double.  Production callers cannot
// use the hook unless they deliberately opt into the test environment.
export function markProbeAgentForTests(agent) {
  if (process.env.NODE_ENV !== 'test') {
    throw new Error('ProbeAgent test marker is unavailable outside NODE_ENV=test');
  }
  return markProbeAgent(agent);
}
