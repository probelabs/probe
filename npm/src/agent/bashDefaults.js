/**
 * Default allow and deny patterns for bash command execution
 * @module agent/bashDefaults
 *
 * Pattern syntax: colon-separated parts matching command + args.
 *   'git:push'        — matches 'git push', 'git push origin main', etc.
 *   'git:push:--force' — matches 'git push --force ...'
 *   'git:branch:*'     — wildcard matches any arg (or no arg) at that position
 *
 * NOTE: 'X' and 'X:*' are functionally identical — the shorter form is preferred.
 * A pattern only checks the parts it specifies; extra args are ignored.
 */

/**
 * Default allow patterns for safe, read-only commands useful for code exploration
 */
export const DEFAULT_ALLOW_PATTERNS = [
  // Basic navigation and listing
  'ls', 'dir', 'pwd', 'cd',

  // File reading commands
  'cat', 'head', 'tail',
  'less', 'more', 'view',

  // File information and metadata
  'file', 'stat', 'wc',
  'du', 'df', 'realpath',

  // Search and find commands (read-only)
  // Note: bare 'find' allows all find variants; dangerous ones (find -exec) are blocked by deny list
  'find',
  'grep', 'egrep', 'fgrep',
  'rg', 'ag', 'ack',
  'which', 'whereis', 'locate',
  'type', 'command',

  // Tree and structure visualization
  'tree',

  // Git read-only operations
  'git:status', 'git:log', 'git:diff',
  'git:show', 'git:branch',
  'git:tag', 'git:describe',
  'git:remote', 'git:config',
  'git:blame', 'git:shortlog', 'git:reflog',
  'git:ls-files', 'git:ls-tree',
  'git:ls-remote',
  'git:rev-parse', 'git:rev-list',
  'git:cat-file',
  'git:diff-tree', 'git:diff-files',
  'git:diff-index',
  'git:for-each-ref',
  'git:merge-base',
  'git:name-rev',
  'git:count-objects',
  'git:verify-commit', 'git:verify-tag',
  'git:check-ignore', 'git:check-attr',
  'git:stash:list', 'git:stash:show',
  'git:worktree:list',
  'git:notes:list', 'git:notes:show',
  'git:--version', 'git:help',

  // GitHub CLI (gh) read-only operations
  'gh:--version', 'gh:help', 'gh:status',
  'gh:auth:status',
  'gh:issue:list', 'gh:issue:view',
  'gh:issue:status',
  'gh:pr:list', 'gh:pr:view',
  'gh:pr:status', 'gh:pr:diff',
  'gh:pr:checks',
  'gh:repo:list', 'gh:repo:view',
  'gh:release:list', 'gh:release:view',
  'gh:run:list', 'gh:run:view',
  'gh:workflow:list', 'gh:workflow:view',
  'gh:gist:list', 'gh:gist:view',
  'gh:search:issues', 'gh:search:prs',
  'gh:search:repos', 'gh:search:code',
  'gh:search:commits',
  'gh:api',

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
  'uname', 'hostname', 'whoami', 'id', 'groups',
  'date', 'cal', 'uptime', 'w', 'users', 'sleep',

  // Environment and shell
  'env', 'printenv', 'echo', 'printf',
  'export', 'set', 'unset',

  // Process information (read-only)
  'ps', 'pgrep', 'jobs', 'top:-n:1',

  // Network information (read-only)
  'ifconfig', 'ip:addr', 'ip:link', 'hostname:-I',
  'ping:-c:*', 'traceroute', 'nslookup', 'dig',

  // Text processing and utilities (awk removed - too powerful)
  'sed:-n:*', 'cut', 'sort',
  'uniq', 'tr', 'column',
  'paste', 'join', 'comm',
  'diff', 'cmp', 'patch:--dry-run:*',

  // Hashing and encoding (read-only)
  'md5sum', 'sha1sum', 'sha256sum',
  'base64', 'base64:-d', 'od', 'hexdump',

  // Archive and compression (list/view only)
  'tar:-tf:*', 'tar:-tzf:*', 'unzip:-l:*', 'zip:-l:*',
  'gzip:-l:*', 'gunzip:-l:*',

  // Help and documentation
  'man', '--help', 'help', 'info',
  'whatis', 'apropos',

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
  'rm:-rf', 'rm:-f:/', 'rm:/', 'rmdir',
  'chmod:777', 'chmod:-R:777', 'chown', 'chgrp',
  'dd', 'shred',

  // Dangerous find operations that can execute arbitrary commands
  'find:-exec', 'find:*:-exec', 'find:-execdir', 'find:*:-execdir',
  'find:-ok', 'find:*:-ok', 'find:-okdir', 'find:*:-okdir',

  // Powerful scripting tools that can execute arbitrary commands
  'awk', 'perl', 'python:-c:*', 'node:-e:*',

  // System administration and modification
  'sudo', 'su',
  'passwd', 'adduser', 'useradd',
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
  'apt', 'apt-get', 'yum', 'dnf', 'zypper',
  'brew:install', 'brew:uninstall', 'brew:upgrade',
  'conda:install', 'conda:remove', 'conda:update',

  // Service and system control
  'systemctl', 'service', 'chkconfig',
  'initctl', 'upstart',

  // Network operations that could be dangerous
  'curl:-d:*', 'curl:--data:*', 'curl:-X:POST:*', 'curl:-X:PUT:*',
  'wget:-O:/', 'wget:--post-data:*',
  'ssh', 'scp', 'sftp', 'rsync',
  'nc', 'netcat', 'telnet',
  'ftp',

  // Process control and termination
  'kill', 'killall', 'pkill',
  'nohup', 'disown',

  // System control and shutdown
  'shutdown', 'reboot', 'halt', 'poweroff',
  'init', 'telinit',

  // Kernel and module operations
  'insmod', 'rmmod', 'modprobe',
  'sysctl:-w:*',

  // Dangerous git operations
  'git:push', 'git:force', 'git:reset',
  'git:clean', 'git:rm',
  'git:commit', 'git:merge',
  'git:rebase', 'git:cherry-pick',
  'git:stash:drop', 'git:stash:pop',
  'git:stash:push', 'git:stash:clear',
  'git:branch:-d', 'git:branch:-D',
  'git:branch:--delete',
  'git:tag:-d', 'git:tag:--delete',
  'git:remote:remove', 'git:remote:rm',
  'git:checkout:--force',
  'git:checkout:-f',
  'git:submodule:deinit',
  'git:notes:add', 'git:notes:remove',
  'git:worktree:add',
  'git:worktree:remove',

  // Dangerous GitHub CLI (gh) write operations
  'gh:issue:create', 'gh:issue:close',
  'gh:issue:delete', 'gh:issue:edit',
  'gh:issue:reopen',
  'gh:issue:comment',
  'gh:pr:create', 'gh:pr:close',
  'gh:pr:merge', 'gh:pr:edit',
  'gh:pr:reopen', 'gh:pr:review',
  'gh:pr:comment',
  'gh:repo:create', 'gh:repo:delete',
  'gh:repo:fork', 'gh:repo:rename',
  'gh:repo:archive', 'gh:repo:clone',
  'gh:release:create', 'gh:release:delete',
  'gh:release:edit',
  'gh:run:cancel', 'gh:run:rerun',
  'gh:workflow:run',
  'gh:workflow:enable', 'gh:workflow:disable',
  'gh:gist:create', 'gh:gist:delete',
  'gh:gist:edit',
  'gh:secret:set', 'gh:secret:delete',
  'gh:variable:set', 'gh:variable:delete',
  'gh:label:create', 'gh:label:delete',
  'gh:ssh-key:add', 'gh:ssh-key:delete',

  // File system mounting and partitioning
  'mount', 'umount', 'fdisk',
  'parted', 'mkfs', 'fsck',

  // Cron and scheduling
  'crontab', 'at', 'batch',

  // Compression with potential overwrite
  'tar:-xf:*', 'unzip', 'gzip', 'gunzip',

  // Build and compilation that might modify files
  'make', 'make:install', 'make:clean', 'cargo:build', 'cargo:install',
  'npm:run:build', 'yarn:build', 'mvn:install', 'gradle:build',

  // Docker operations that could modify state
  'docker:run', 'docker:exec',
  'docker:build', 'docker:pull', 'docker:push',
  'docker:rm', 'docker:rmi', 'docker:stop', 'docker:start',

  // Database operations
  'mysql:-e:DROP', 'psql:-c:DROP', 'redis-cli:FLUSHALL',
  'mongo:--eval:*',

  // Text editors that could modify files
  'vi', 'vim', 'nano', 'emacs',
  'sed:-i:*', 'perl:-i:*',

  // Potentially dangerous utilities
  'eval', 'exec', 'source',
  'bash:-c:*', 'sh:-c:*', 'zsh:-c:*'
];
