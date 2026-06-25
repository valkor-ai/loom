export type LoomCommandSurface = "@loom" | "/loom";

export function completedDeliveryUserMessage(commandSurface?: LoomCommandSurface): string {
  const newDeliveryHint = commandSurface
    ? `如果要开始新的交付，请重新发起一条新的 Loom 需求，例如：${commandSurface} 实现新的需求。`
    : "如果要开始新的交付，请重新发起一条新的 Loom 需求：Codex 用 @loom 实现新的需求；Claude/opencode 用 /loom 实现新的需求。";
  return `当前交付已经完成，不是卡住状态；无需再执行 continue。${newDeliveryHint}`;
}
