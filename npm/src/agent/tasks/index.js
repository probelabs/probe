/**
 * Task Management Module
 * @module agent/tasks
 */

export { TaskManager, default as TaskManagerDefault } from './TaskManager.js';
export {
  taskSchema,
  taskSystemPrompt,
  taskGuidancePrompt,
  createTaskCompletionBlockedMessage,
  createTaskTool,
  default as createTaskToolDefault
} from './taskTool.js';
