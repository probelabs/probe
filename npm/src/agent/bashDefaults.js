/**
 * Default allow and deny patterns for bash command execution
 * @module agent/bashDefaults
 */

/**
 * Default allow patterns for safe, read-only commands useful for code exploration
 */
export const DEFAULT_ALLOW_PATTERNS = [
  // Basic navigation and listing
  'ls', 'ls:*', 'dir', 'pwd', 'cd', 'cd:*',
  
  // File reading commands
  'cat', 'cat:*', 'head', 'head:*', 'tail', 'tail:*',
  'less', 'more', 'view',
  
  // File information and metadata
  'file', 'file:*', 'stat', 'stat:*', 'wc', 'wc:*',
  'du', 'du:*', 'df', 'df:*', 'realpath', 'realpath:*',
  
  // Search and find commands (read-only)
  'find', 'find:*', 'grep', 'grep:*', 'egrep', 'egrep:*', 'fgrep', 'fgrep:*',
  'rg', 'rg:*', 'ag', 'ag:*', 'ack', 'ack:*',
  'which', 'which:*', 'whereis', 'whereis:*', 'locate', 'locate:*',
  'type', 'type:*', 'command', 'command:*',
  
  // Tree and structure visualization
  'tree', 'tree:*',
  
  // Git read-only operations
  'git:status', 'git:log', 'git:log:*', 'git:diff', 'git:diff:*',
  'git:show', 'git:show:*', 'git:branch', 'git:branch:*',
  'git:tag', 'git:tag:*', 'git:describe', 'git:describe:*',
  'git:remote', 'git:remote:*', 'git:config:*',
  'git:blame', 'git:blame:*', 'git:shortlog', 'git:reflog',
  'git:ls-files', 'git:ls-tree', 'git:rev-parse', 'git:rev-list',
  'git:--version', 'git:help', 'git:help:*',
  
  // Package managers (information only)
  'npm:list', 'npm:ls', 'npm:view', 'npm:info', 'npm:show',
  'npm:outdated', 'npm:audit', 'npm:--version',
  'yarn:list', 'yarn:info', 'yarn:--version',
  'pnpm:list', 'pnpm:--version',
  'pip:list', 'pip:show', 'pip:--version',
  'pip3:list', 'pip3:show', 'pip3:--version',
  'gem:list', 'gem:--version',
  'bundle:list', 'bundle:show', 'bundle:--version',
  'composer:show', 'composer:--version',
  
  // Language and runtime versions
  'node:--version', 'node:-v',
  'python:--version', 'python:-V', 'python3:--version', 'python3:-V',
  'ruby:--version', 'ruby:-v',
  'go:version', 'go:env', 'go:list', 'go:mod:graph',
  'rustc:--version', 'cargo:--version', 'cargo:tree', 'cargo:metadata',
  'java:--version', 'java:-version', 'javac:--version',
  'mvn:--version', 'gradle:--version',
  'php:--version', 'dotnet:--version', 'dotnet:list',
  
  // Database client versions (connection info only)
  'psql:--version', 'mysql:--version', 'redis-cli:--version',
  'mongo:--version', 'sqlite3:--version',
  
  // System information
  'uname', 'uname:*', 'hostname', 'whoami', 'id', 'groups',
  'date', 'cal', 'uptime', 'w', 'users',
  
  // Environment and shell
  'env', 'printenv', 'echo', 'echo:*', 'printf', 'printf:*',
  'export', 'export:*', 'set', 'unset',
  
  // Process information (read-only)
  'ps', 'ps:*', 'pgrep', 'pgrep:*', 'jobs', 'top:-n:1',
  
  // Network information (read-only)
  'ifconfig', 'ip:addr', 'ip:link', 'hostname:-I',
  'ping:-c:*', 'traceroute', 'nslookup', 'dig',
  
  // Text processing and utilities
  'awk', 'awk:*', 'sed:-n:*', 'cut', 'cut:*', 'sort', 'sort:*',
  'uniq', 'uniq:*', 'tr', 'tr:*', 'column', 'column:*',
  'paste', 'paste:*', 'join', 'join:*', 'comm', 'comm:*',
  'diff', 'diff:*', 'cmp', 'cmp:*', 'patch:--dry-run:*',
  
  // Hashing and encoding (read-only)
  'md5sum', 'md5sum:*', 'sha1sum', 'sha1sum:*', 'sha256sum', 'sha256sum:*',
  'base64', 'base64:-d', 'od', 'od:*', 'hexdump', 'hexdump:*',
  
  // Archive and compression (list/view only)
  'tar:-tf:*', 'tar:-tzf:*', 'unzip:-l:*', 'zip:-l:*',
  'gzip:-l:*', 'gunzip:-l:*',
  
  // Help and documentation
  'man', 'man:*', '--help', 'help', 'info', 'info:*',
  'whatis', 'whatis:*', 'apropos', 'apropos:*',
  
  // Make (dry run and info)
  'make:-n', 'make:--dry-run', 'make:-p', 'make:--print-data-base',
  
  // Docker (read-only operations)
  'docker:ps', 'docker:images', 'docker:version', 'docker:info',
  'docker:logs:*', 'docker:inspect:*',
  
  // Test runners (list/info only)
  'jest:--listTests', 'mocha:--help', 'pytest:--collect-only'
];

/**
 * Default deny patterns for potentially dangerous or destructive commands
 */
export const DEFAULT_DENY_PATTERNS = [
  // Dangerous file operations
  'rm:-rf', 'rm:-f:/', 'rm:/', 'rm:-rf:*', 'rmdir', 
  'chmod:777', 'chmod:-R:777', 'chown', 'chgrp',
  'dd', 'dd:*', 'shred', 'shred:*',
  
  // System administration and modification
  'sudo:*', 'su', 'su:*', 'passwd', 'adduser', 'useradd',
  'userdel', 'usermod', 'groupadd', 'groupdel', 'visudo',
  
  // Package installation and removal
  'npm:install', 'npm:i', 'npm:uninstall', 'npm:publish',
  'npm:unpublish', 'npm:link', 'npm:update',
  'yarn:install', 'yarn:add', 'yarn:remove', 'yarn:upgrade',
  'pnpm:install', 'pnpm:add', 'pnpm:remove',
  'pip:install', 'pip:uninstall', 'pip:upgrade',
  'pip3:install', 'pip3:uninstall', 'pip3:upgrade',
  'gem:install', 'gem:uninstall', 'gem:update',
  'bundle:install', 'bundle:update',
  'composer:install', 'composer:update', 'composer:remove',
  'apt:*', 'apt-get:*', 'yum:*', 'dnf:*', 'zypper:*',
  'brew:install', 'brew:uninstall', 'brew:upgrade',
  'conda:install', 'conda:remove', 'conda:update',
  
  // Service and system control
  'systemctl:*', 'service:*', 'chkconfig:*',
  'initctl:*', 'upstart:*',
  
  // Network operations that could be dangerous
  'curl:-d:*', 'curl:--data:*', 'curl:-X:POST:*', 'curl:-X:PUT:*',
  'wget:-O:/', 'wget:--post-data:*',
  'ssh', 'ssh:*', 'scp', 'scp:*', 'sftp', 'sftp:*', 'rsync:*',
  'nc', 'nc:*', 'netcat', 'netcat:*', 'telnet', 'telnet:*',
  'ftp', 'ftp:*',
  
  // Process control and termination
  'kill', 'kill:*', 'killall', 'killall:*', 'pkill', 'pkill:*',
  'nohup:*', 'disown:*',
  
  // System control and shutdown
  'shutdown', 'shutdown:*', 'reboot', 'halt', 'poweroff',
  'init', 'telinit',
  
  // Kernel and module operations
  'insmod', 'insmod:*', 'rmmod', 'rmmod:*', 'modprobe', 'modprobe:*',
  'sysctl:-w:*',
  
  // Dangerous git operations
  'git:push', 'git:push:*', 'git:force', 'git:reset:--hard:*',
  'git:clean:-fd', 'git:rm:*', 'git:commit', 'git:merge',
  'git:rebase', 'git:cherry-pick', 'git:stash:drop',
  
  // File system mounting and partitioning
  'mount', 'mount:*', 'umount', 'umount:*', 'fdisk', 'fdisk:*',
  'parted', 'parted:*', 'mkfs', 'mkfs:*', 'fsck', 'fsck:*',
  
  // Cron and scheduling
  'crontab', 'crontab:*', 'at', 'at:*', 'batch', 'batch:*',
  
  // Compression with potential overwrite
  'tar:-xf:*', 'unzip', 'unzip:*', 'gzip:*', 'gunzip:*',
  
  // Build and compilation that might modify files
  'make', 'make:install', 'make:clean', 'cargo:build', 'cargo:install',
  'npm:run:build', 'yarn:build', 'mvn:install', 'gradle:build',
  
  // Docker operations that could modify state
  'docker:run', 'docker:run:*', 'docker:exec', 'docker:exec:*',
  'docker:build', 'docker:build:*', 'docker:pull', 'docker:push',
  'docker:rm', 'docker:rmi', 'docker:stop', 'docker:start',
  
  // Database operations
  'mysql:-e:DROP', 'psql:-c:DROP', 'redis-cli:FLUSHALL',
  'mongo:--eval:*',
  
  // Text editors that could modify files
  'vi', 'vi:*', 'vim', 'vim:*', 'nano', 'nano:*', 'emacs', 'emacs:*',
  'sed:-i:*', 'perl:-i:*',
  
  // Potentially dangerous utilities
  'eval', 'eval:*', 'exec', 'exec:*', 'source', 'source:*',
  'bash:-c:*', 'sh:-c:*', 'zsh:-c:*'
];