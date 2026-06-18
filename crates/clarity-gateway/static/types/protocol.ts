/**
 * TypeScript types for the Clarity Gateway WebSocket protocol.
 *
 * The Gateway speaks a single JSON envelope (`WsResponse`) on `/ws`.
 * Streaming agent events are wrapped in `WireMessage` payloads rather than
 * emitted as raw `clarity_wire` messages.
 */

/** Client → Gateway request envelope. */
export type WsRequest =
    | ChatRequest
    | PingRequest
    | GetHistoryRequest;

export interface ChatRequest {
    type: 'chat';
    /** Message text from the user. */
    message: string;
    /** Optional request context. */
    context?: Record<string, unknown>;
    /**
     * Whether to stream wire events.
     *
     * When true, the server emits `WsResponse.WireMessage` envelopes.
     */
    use_wire?: boolean;
}

export interface PingRequest {
    type: 'ping';
}

export interface GetHistoryRequest {
    type: 'get_history';
}

/** Gateway → Client response envelope. */
export type WsResponse =
    | WelcomeResponse
    | ChatResponse
    | PongResponse
    | HistoryResponse
    | ErrorResponse
    | WireMessageResponse;

export interface WelcomeResponse {
    type: 'welcome';
    session_id: string;
    message: string;
}

export interface ChatResponse {
    type: 'chat';
    message: string;
    tool_calls?: ToolCall[];
}

export interface PongResponse {
    type: 'pong';
}

export interface HistoryResponse {
    type: 'history';
    messages: ChatMessage[];
}

export interface ErrorResponse {
    type: 'error';
    error: string;
}

/** Streaming WireMessage wrapped in the unified WsResponse envelope. */
export interface WireMessageResponse {
    type: 'wire_message';
    payload: WireMessagePayload;
}

export interface ToolCall {
    name: string;
    arguments: unknown;
}

export interface ChatMessage {
    role: string;
    content: string;
    timestamp: string;
}

/** All clarity_wire::WireMessage variants emitted during a streaming turn. */
export type WireMessagePayload =
    | TurnBeginWireMessage
    | StepBeginWireMessage
    | ContentPartWireMessage
    | DraftEventWireMessage
    | ToolCallWireMessage
    | ToolResultWireMessage
    | TurnEndWireMessage
    | UsageWireMessage
    | StatusUpdateWireMessage
    | ViewStateUpdateWireMessage
    | CompactionBeginWireMessage
    | CompactionEndWireMessage
    | PlanStepBeginWireMessage
    | PlanStepEndWireMessage
    | PlanStepSkippedWireMessage
    | ThreadActiveWireMessage
    | ThreadListWireMessage
    | ThreadCreatedWireMessage
    | ThreadUpdatedWireMessage;

interface BaseWireMessage {
    /** Identifier for the turn this message belongs to. */
    turn_id?: string;
}

export interface TurnBeginWireMessage extends BaseWireMessage {
    type: 'turn_begin';
    user_input: string;
}

export interface StepBeginWireMessage extends BaseWireMessage {
    type: 'step_begin';
    tool_name: string;
}

export interface ContentPartWireMessage extends BaseWireMessage {
    type: 'content_part';
    text: string;
}

export interface DraftEventWireMessage extends BaseWireMessage {
    type: 'draft_event';
    event: DraftEvent;
}

export type DraftEvent =
    | { type: 'clear' }
    | { type: 'progress'; text: string }
    | { type: 'content'; text: string };

export interface ToolCallWireMessage extends BaseWireMessage {
    type: 'tool_call';
    id: string;
    name: string;
    arguments: unknown;
}

export interface ToolResultWireMessage extends BaseWireMessage {
    type: 'tool_result';
    id: string;
    result: string;
}

export interface TurnEndWireMessage extends BaseWireMessage {
    type: 'turn_end';
}

export interface UsageWireMessage extends BaseWireMessage {
    type: 'usage';
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
}

export interface StatusUpdateWireMessage extends BaseWireMessage {
    type: 'status_update';
    message: string;
}

export interface ViewStateUpdateWireMessage extends BaseWireMessage {
    type: 'view_state_update';
    turn?: TurnState;
}

export type TurnState = 'idle' | 'loading' | 'compacting' | 'stopping' | 'restoring';

export interface CompactionBeginWireMessage extends BaseWireMessage {
    type: 'compaction_begin';
}

export interface CompactionEndWireMessage extends BaseWireMessage {
    type: 'compaction_end';
}

export interface PlanStepBeginWireMessage extends BaseWireMessage {
    type: 'plan_step_begin';
    step_id: string;
    tool_name: string;
}

export interface PlanStepEndWireMessage extends BaseWireMessage {
    type: 'plan_step_end';
    step_id: string;
    success: boolean;
}

export interface PlanStepSkippedWireMessage extends BaseWireMessage {
    type: 'plan_step_skipped';
    step_id: string;
}

export interface ThreadActiveWireMessage {
    type: 'thread_active';
    thread_id: string;
    title?: string;
}

export interface ThreadListWireMessage {
    type: 'thread_list';
    threads: ThreadSummary[];
}

export interface ThreadCreatedWireMessage {
    type: 'thread_created';
    thread_id: string;
    title?: string;
}

export interface ThreadUpdatedWireMessage {
    type: 'thread_updated';
    thread_id: string;
    title?: string;
    archived?: boolean;
}

export interface ThreadSummary {
    thread_id: string;
    title?: string;
    archived?: boolean;
}

/** Convenience dispatcher for client-side routing. */
export function isWireMessage(response: WsResponse): response is WireMessageResponse {
    return response.type === 'wire_message';
}
