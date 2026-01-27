function escapeXml(value) {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&apos;');
}

export function formatAvailableSkillsXml(skills) {
  if (!skills || skills.length === 0) return '';

  const lines = ['<available_skills>'];
  for (const skill of skills) {
    lines.push('  <skill>');
    lines.push(`    <name>${escapeXml(skill.name)}</name>`);
    lines.push(`    <description>${escapeXml(skill.description || '')}</description>`);
    lines.push('  </skill>');
  }
  lines.push('</available_skills>');

  return lines.join('\n');
}
