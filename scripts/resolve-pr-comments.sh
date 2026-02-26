#!/bin/bash
# resolve-pr-comments.sh — List, reply to, and resolve PR review comments
#
# Usage:
#   ./scripts/resolve-pr-comments.sh <PR#> list                          # List unresolved threads
#   ./scripts/resolve-pr-comments.sh <PR#> list-all                      # List ALL threads (including resolved)
#   ./scripts/resolve-pr-comments.sh <PR#> reply <index> "message"       # Reply + resolve unresolved thread index
#   ./scripts/resolve-pr-comments.sh <PR#> resolve <index>               # Resolve unresolved thread index (no reply)
#   ./scripts/resolve-pr-comments.sh <PR#> resolve-all                   # Resolve ALL unresolved threads
#   ./scripts/resolve-pr-comments.sh <PR#> smoke-test                    # Validate list/resolve paths without mutating
#
# Requires: gh CLI authenticated with repo access

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
DIM='\033[0;90m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Retry controls for transient GitHub API failures
GH_MAX_RETRIES="${GH_MAX_RETRIES:-4}"
GH_RETRY_BASE_DELAY_SECONDS="${GH_RETRY_BASE_DELAY_SECONDS:-1}"

# --- Helpers ---

usage() {
    echo -e "${BOLD}Usage:${NC}"
    echo "  $0 <PR#> list                       List unresolved review threads"
    echo "  $0 <PR#> list-all                    List ALL review threads (including resolved)"
    echo "  $0 <PR#> reply <index> \"message\"     Reply to unresolved thread index and resolve it"
    echo "  $0 <PR#> resolve <index>             Resolve unresolved thread index without replying"
    echo "  $0 <PR#> resolve-all                 Resolve ALL unresolved threads"
    echo "  $0 <PR#> smoke-test                  Validate list/resolve paths (dry-run, no mutation)"
    exit 1
}

get_repo() {
    gh repo view --json nameWithOwner --jq '.nameWithOwner' 2>/dev/null || {
        echo -e "${RED}Error: Could not detect repo. Run from a git repo with gh configured.${NC}" >&2
        exit 1
    }
}

require_command() {
    local cmd="$1"
    if ! command -v "$cmd" >/dev/null 2>&1; then
        echo -e "${RED}Error: Required command '$cmd' was not found in PATH.${NC}" >&2
        exit 1
    fi
}

is_retryable_error() {
    local message="$1"
    local lower
    lower=$(echo "$message" | tr '[:upper:]' '[:lower:]')

    case "$lower" in
        *"timeout"*|*"temporary"*|*"connection reset"*|*"connection refused"*|*"bad gateway"*|*"service unavailable"*|*"internal server error"*|*"rate limit"*|*"502"*|*"503"*|*"504"*)
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

gh_api_request() {
    local attempt=1
    local delay="$GH_RETRY_BASE_DELAY_SECONDS"
    local output=""
    local status=0

    while [ "$attempt" -le "$GH_MAX_RETRIES" ]; do
        set +e
        output=$(gh api "$@" 2>&1)
        status=$?
        set -e

        if [ "$status" -eq 0 ]; then
            echo "$output"
            return 0
        fi

        if [ "$attempt" -lt "$GH_MAX_RETRIES" ] && is_retryable_error "$output"; then
            echo -e "${YELLOW}GitHub API request attempt $attempt/$GH_MAX_RETRIES failed. Retrying in ${delay}s...${NC}" >&2
            sleep "$delay"
            attempt=$((attempt + 1))
            delay=$((delay * 2))
            continue
        fi

        echo -e "${RED}GitHub API request failed:${NC}" >&2
        echo "$output" >&2
        return "$status"
    done

    echo -e "${RED}GitHub API request failed after $GH_MAX_RETRIES attempt(s).${NC}" >&2
    return 1
}

graphql_request() {
    local query="$1"
    shift

    local attempt=1
    local delay="$GH_RETRY_BASE_DELAY_SECONDS"
    local output=""
    local status=0

    while [ "$attempt" -le "$GH_MAX_RETRIES" ]; do
        set +e
        output=$(gh api graphql -f query="$query" "$@" 2>&1)
        status=$?
        set -e

        if [ "$status" -eq 0 ]; then
            local has_graphql_errors
            has_graphql_errors=$(echo "$output" | jq -r '((.errors // []) | length) > 0' 2>/dev/null || echo "parse-error")

            if [ "$has_graphql_errors" = "false" ]; then
                echo "$output"
                return 0
            fi

            if [ "$has_graphql_errors" = "true" ]; then
                local graphql_error_messages
                graphql_error_messages=$(echo "$output" | jq -r '[.errors[]?.message] | join("; ")')

                if [ "$attempt" -lt "$GH_MAX_RETRIES" ] && is_retryable_error "$graphql_error_messages"; then
                    echo -e "${YELLOW}GraphQL attempt $attempt/$GH_MAX_RETRIES failed: ${graphql_error_messages}. Retrying in ${delay}s...${NC}" >&2
                    sleep "$delay"
                    attempt=$((attempt + 1))
                    delay=$((delay * 2))
                    continue
                fi

                echo -e "${RED}GraphQL error: ${graphql_error_messages}${NC}" >&2
                return 1
            fi

            if [ "$attempt" -lt "$GH_MAX_RETRIES" ] && is_retryable_error "$output"; then
                echo -e "${YELLOW}GraphQL response parse failed on attempt $attempt/$GH_MAX_RETRIES. Retrying in ${delay}s...${NC}" >&2
                sleep "$delay"
                attempt=$((attempt + 1))
                delay=$((delay * 2))
                continue
            fi

            echo -e "${RED}GraphQL returned a non-JSON response:${NC}" >&2
            echo "$output" >&2
            return 1
        fi

        if [ "$attempt" -lt "$GH_MAX_RETRIES" ] && is_retryable_error "$output"; then
            echo -e "${YELLOW}GraphQL request attempt $attempt/$GH_MAX_RETRIES failed. Retrying in ${delay}s...${NC}" >&2
            sleep "$delay"
            attempt=$((attempt + 1))
            delay=$((delay * 2))
            continue
        fi

        echo -e "${RED}GraphQL request failed:${NC}" >&2
        echo "$output" >&2
        return "$status"
    done

    echo -e "${RED}GraphQL request failed after $GH_MAX_RETRIES attempt(s).${NC}" >&2
    return 1
}

# Fetch all review threads as JSON array
# Each element: { id, isResolved, restCommentId, author, path, line, body }
fetch_threads() {
    local pr_number="$1"
    local repo
    repo=$(get_repo)
    local owner="${repo%/*}"
    local name="${repo#*/}"

    local query
    query=$(cat <<'EOF'
query($owner: String!, $name: String!, $prNumber: Int!, $after: String) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $prNumber) {
      reviewThreads(first: 100, after: $after) {
        pageInfo {
          hasNextPage
          endCursor
        }
        nodes {
          id
          isResolved
          comments(first: 1) {
            nodes {
              databaseId
              author { login }
              path
              line
              originalLine
              body
            }
          }
        }
      }
    }
  }
}
EOF
)

    local all_threads='[]'
    local after='null'

    while true; do
        local response
        if [ "$after" = "null" ]; then
            response=$(graphql_request "$query" -f owner="$owner" -f name="$name" -F prNumber="$pr_number" -F after=null)
        else
            response=$(graphql_request "$query" -f owner="$owner" -f name="$name" -F prNumber="$pr_number" -f after="$after")
        fi

        local pr_exists
        pr_exists=$(echo "$response" | jq -r '.data.repository.pullRequest != null')
        if [ "$pr_exists" != "true" ]; then
            echo -e "${RED}Error: PR #$pr_number not found in $repo.${NC}" >&2
            return 1
        fi

        local page_threads
        page_threads=$(echo "$response" | jq '.data.repository.pullRequest.reviewThreads.nodes')
        all_threads=$(jq -cn --argjson existing "$all_threads" --argjson page "$page_threads" '$existing + $page')

        local has_next_page
        has_next_page=$(echo "$response" | jq -r '.data.repository.pullRequest.reviewThreads.pageInfo.hasNextPage')
        if [ "$has_next_page" != "true" ]; then
            break
        fi

        after=$(echo "$response" | jq -r '.data.repository.pullRequest.reviewThreads.pageInfo.endCursor')
        if [ -z "$after" ] || [ "$after" = "null" ]; then
            echo -e "${RED}Error: Missing pagination cursor for additional review thread pages.${NC}" >&2
            return 1
        fi
    done

    echo "$all_threads" | jq '[.[] | .comments.nodes[0] as $comment | {
        id: .id,
        isResolved: .isResolved,
        restCommentId: ($comment.databaseId // null),
        author: ($comment.author.login // "unknown"),
        path: ($comment.path // "(unknown)"),
        line: ($comment.line // $comment.originalLine // "n/a"),
        body: ($comment.body // "")
    }]'
}

# Resolve a thread by its GraphQL node ID
resolve_thread() {
    local thread_id="$1"
    local mutation
    mutation=$(cat <<'EOF'
mutation($threadId: ID!) {
  resolveReviewThread(input: {threadId: $threadId}) {
    thread { isResolved }
  }
}
EOF
)

    local response
    response=$(graphql_request "$mutation" -f threadId="$thread_id")
    echo "$response" | jq -r '.data.resolveReviewThread.thread.isResolved'
}

# Reply to a comment by its REST database ID
reply_to_comment() {
    local pr_number="$1"
    local comment_id="$2"
    local message="$3"
    local repo
    repo=$(get_repo)

    local response
    response=$(gh_api_request "repos/$repo/pulls/$pr_number/comments/$comment_id/replies" -f body="$message")
    echo "$response" | jq -r '.id // empty'
}

# Truncate string to max length with ellipsis
truncate() {
    local str="$1"
    local max="${2:-100}"
    # Remove newlines for display
    str=$(echo "$str" | tr '\n' ' ' | sed 's/  */ /g')
    if [ "${#str}" -gt "$max" ]; then
        echo "${str:0:$max}..."
    else
        echo "$str"
    fi
}

validate_index() {
    local value="$1"
    local label="$2"
    if ! [[ "$value" =~ ^[0-9]+$ ]]; then
        echo -e "${RED}Error: $label must be a non-negative integer, got '$value'.${NC}" >&2
        exit 1
    fi
}

filter_threads() {
    local threads="$1"
    local include_resolved="${2:-false}"
    if [ "$include_resolved" = "true" ]; then
        echo "$threads"
    else
        echo "$threads" | jq '[.[] | select(.isResolved == false)]'
    fi
}

resolve_dry_run_check() {
    local thread_id="$1"
    local query
    query=$(cat <<'EOF'
query($threadId: ID!) {
  node(id: $threadId) {
    __typename
    ... on PullRequestReviewThread {
      id
      isResolved
      isOutdated
    }
  }
}
EOF
)

    local response
    response=$(graphql_request "$query" -f threadId="$thread_id")

    local typename
    typename=$(echo "$response" | jq -r '.data.node.__typename // empty')
    if [ "$typename" != "PullRequestReviewThread" ]; then
        echo -e "${RED}Dry-run check failed: expected PullRequestReviewThread, got '${typename:-<empty>}'.${NC}" >&2
        return 1
    fi

    echo "$response" | jq -r '.data.node.isResolved'
}

# --- Commands ---

cmd_list() {
    local pr_number="$1"
    local show_resolved="${2:-false}"
    local threads filtered_threads
    threads=$(fetch_threads "$pr_number")
    filtered_threads=$(filter_threads "$threads" "$show_resolved")

    local count
    count=$(echo "$filtered_threads" | jq 'length')

    if [ "$count" -eq 0 ]; then
        if [ "$show_resolved" = "true" ]; then
            echo -e "${DIM}No review threads found on PR #$pr_number.${NC}"
        else
            echo -e "${DIM}No unresolved review threads found on PR #$pr_number.${NC}"
        fi
        return
    fi

    local idx=0

    echo -e "${BOLD}Review threads on PR #$pr_number:${NC}"
    if [ "$show_resolved" = "false" ]; then
        echo -e "${DIM}Indices below are unresolved-only and map directly to 'reply'/'resolve'.${NC}"
    fi
    echo ""

    while [ "$idx" -lt "$count" ]; do
        local resolved author path line body
        resolved=$(echo "$filtered_threads" | jq -r ".[$idx].isResolved")
        author=$(echo "$filtered_threads" | jq -r ".[$idx].author")
        path=$(echo "$filtered_threads" | jq -r ".[$idx].path")
        line=$(echo "$filtered_threads" | jq -r ".[$idx].line")
        body=$(echo "$filtered_threads" | jq -r ".[$idx].body")

        local status_icon status_color
        if [ "$resolved" = "true" ]; then
            status_icon="[resolved]"
            status_color="$DIM"
        else
            status_icon="[open]"
            status_color="$YELLOW"
        fi

        local display_body
        display_body=$(truncate "$body" 120)

        echo -e "  ${CYAN}#$idx${NC}  ${status_color}${status_icon}${NC}  ${BLUE}${author}${NC}  ${path}:${line}"
        echo -e "      ${DIM}${display_body}${NC}"
        echo ""

        idx=$((idx + 1))
    done

    echo -e "${DIM}Showing $count thread(s). Use index number with 'reply' or 'resolve' commands.${NC}"
}

cmd_reply() {
    local pr_number="$1"
    local target_idx="$2"
    local message="$3"
    validate_index "$target_idx" "Index"

    local threads actionable_threads
    threads=$(fetch_threads "$pr_number")
    actionable_threads=$(filter_threads "$threads" false)

    local count
    count=$(echo "$actionable_threads" | jq 'length')

    if [ "$count" -eq 0 ]; then
        echo -e "${YELLOW}No unresolved review threads found on PR #$pr_number.${NC}"
        exit 1
    fi

    if [ "$target_idx" -ge "$count" ]; then
        echo -e "${RED}Error: Index #$target_idx out of range for unresolved threads (0-$((count - 1))).${NC}" >&2
        exit 1
    fi

    local thread_id rest_id author path line
    thread_id=$(echo "$actionable_threads" | jq -r ".[$target_idx].id")
    rest_id=$(echo "$actionable_threads" | jq -r ".[$target_idx].restCommentId")
    author=$(echo "$actionable_threads" | jq -r ".[$target_idx].author")
    path=$(echo "$actionable_threads" | jq -r ".[$target_idx].path")
    line=$(echo "$actionable_threads" | jq -r ".[$target_idx].line")

    if [ "$rest_id" = "null" ] || [ -z "$rest_id" ]; then
        echo -e "${RED}Error: Cannot reply to thread #$target_idx because no REST comment ID is available.${NC}" >&2
        exit 1
    fi

    echo -e "${BOLD}Replying to thread #$target_idx${NC} (${BLUE}${author}${NC} on ${path}:${line})..."

    # Post reply
    local reply_id
    reply_id=$(reply_to_comment "$pr_number" "$rest_id" "$message")

    if [ -n "$reply_id" ]; then
        echo -e "  ${GREEN}Reply posted${NC} (comment ID: $reply_id)"
    else
        echo -e "  ${RED}Failed to post reply${NC}" >&2
        exit 1
    fi

    # Resolve thread
    local result
    result=$(resolve_thread "$thread_id")
    if [ "$result" = "true" ]; then
        echo -e "  ${GREEN}Thread resolved${NC}"
    else
        echo -e "  ${YELLOW}Thread may already be resolved${NC}"
    fi
}

cmd_resolve() {
    local pr_number="$1"
    local target_idx="$2"
    validate_index "$target_idx" "Index"

    local threads actionable_threads
    threads=$(fetch_threads "$pr_number")
    actionable_threads=$(filter_threads "$threads" false)

    local count
    count=$(echo "$actionable_threads" | jq 'length')

    if [ "$count" -eq 0 ]; then
        echo -e "${YELLOW}No unresolved review threads found on PR #$pr_number.${NC}"
        exit 1
    fi

    if [ "$target_idx" -ge "$count" ]; then
        echo -e "${RED}Error: Index #$target_idx out of range for unresolved threads (0-$((count - 1))).${NC}" >&2
        exit 1
    fi

    local thread_id author path line
    thread_id=$(echo "$actionable_threads" | jq -r ".[$target_idx].id")
    author=$(echo "$actionable_threads" | jq -r ".[$target_idx].author")
    path=$(echo "$actionable_threads" | jq -r ".[$target_idx].path")
    line=$(echo "$actionable_threads" | jq -r ".[$target_idx].line")

    echo -e "${BOLD}Resolving thread #$target_idx${NC} (${BLUE}${author}${NC} on ${path}:${line})..."

    local result
    result=$(resolve_thread "$thread_id")
    if [ "$result" = "true" ]; then
        echo -e "  ${GREEN}Thread resolved${NC}"
    else
        echo -e "  ${YELLOW}Thread may already be resolved${NC}"
    fi
}

cmd_smoke_test() {
    local pr_number="$1"
    local threads unresolved_threads
    threads=$(fetch_threads "$pr_number")
    unresolved_threads=$(filter_threads "$threads" false)

    local total_count unresolved_count
    total_count=$(echo "$threads" | jq 'length')
    unresolved_count=$(echo "$unresolved_threads" | jq 'length')

    echo -e "${BOLD}Smoke test for PR #$pr_number${NC}"
    echo -e "  ${GREEN}Fetched review threads${NC}: total=$total_count unresolved=$unresolved_count"

    if [ "$total_count" -eq 0 ]; then
        echo -e "  ${DIM}No threads available. Fetch path validated.${NC}"
        return
    fi

    local candidate
    candidate=$(echo "$threads" | jq '([.[] | select(.isResolved == false)][0] // .[0])')

    local thread_id author path line action_idx
    thread_id=$(echo "$candidate" | jq -r '.id')
    author=$(echo "$candidate" | jq -r '.author')
    path=$(echo "$candidate" | jq -r '.path')
    line=$(echo "$candidate" | jq -r '.line')
    action_idx=$(echo "$unresolved_threads" | jq -r --arg thread_id "$thread_id" 'map(.id) | index($thread_id)')

    local dry_run_state
    dry_run_state=$(resolve_dry_run_check "$thread_id")

    echo -e "  ${GREEN}Resolve dry-run check passed${NC}: threadId=$thread_id isResolved=$dry_run_state"
    if [ "$action_idx" = "null" ]; then
        echo -e "  ${DIM}Selected thread is already resolved; no unresolved action index exists.${NC}"
    else
        echo -e "  ${DIM}Would resolve unresolved index #$action_idx (${author} on ${path}:${line})${NC}"
    fi
    echo -e "  ${DIM}No mutation executed.${NC}"
}

cmd_resolve_all() {
    local pr_number="$1"
    local threads
    threads=$(fetch_threads "$pr_number")

    local count
    count=$(echo "$threads" | jq 'length')
    local resolved_count=0

    echo -e "${BOLD}Resolving all unresolved threads on PR #$pr_number...${NC}"

    local idx=0
    while [ "$idx" -lt "$count" ]; do
        local resolved thread_id author path line
        resolved=$(echo "$threads" | jq -r ".[$idx].isResolved")

        if [ "$resolved" = "false" ]; then
            thread_id=$(echo "$threads" | jq -r ".[$idx].id")
            author=$(echo "$threads" | jq -r ".[$idx].author")
            path=$(echo "$threads" | jq -r ".[$idx].path")
            line=$(echo "$threads" | jq -r ".[$idx].line")

            local result
            result=$(resolve_thread "$thread_id")
            if [ "$result" = "true" ]; then
                echo -e "  ${GREEN}Resolved${NC} #$idx (${BLUE}${author}${NC} on ${path}:${line})"
                resolved_count=$((resolved_count + 1))
            else
                echo -e "  ${RED}Failed${NC} #$idx" >&2
            fi
        fi

        idx=$((idx + 1))
    done

    if [ "$resolved_count" -eq 0 ]; then
        echo -e "  ${DIM}No unresolved threads found.${NC}"
    else
        echo -e "${GREEN}Resolved $resolved_count thread(s).${NC}"
    fi
}

# --- Main ---

if [ $# -lt 2 ]; then
    usage
fi

PR_NUMBER="$1"
COMMAND="$2"

# Validate PR number is numeric
if ! [[ "$PR_NUMBER" =~ ^[0-9]+$ ]]; then
    echo -e "${RED}Error: PR number must be numeric, got '$PR_NUMBER'${NC}" >&2
    usage
fi

require_command gh
require_command jq

case "$COMMAND" in
    list)
        cmd_list "$PR_NUMBER" false
        ;;
    list-all)
        cmd_list "$PR_NUMBER" true
        ;;
    reply)
        if [ $# -lt 4 ]; then
            echo -e "${RED}Error: 'reply' requires an index and message${NC}" >&2
            echo "  Usage: $0 $PR_NUMBER reply <index> \"message\""
            exit 1
        fi
        cmd_reply "$PR_NUMBER" "$3" "$4"
        ;;
    resolve)
        if [ $# -lt 3 ]; then
            echo -e "${RED}Error: 'resolve' requires an index${NC}" >&2
            echo "  Usage: $0 $PR_NUMBER resolve <index>"
            exit 1
        fi
        cmd_resolve "$PR_NUMBER" "$3"
        ;;
    resolve-all)
        cmd_resolve_all "$PR_NUMBER"
        ;;
    smoke-test)
        cmd_smoke_test "$PR_NUMBER"
        ;;
    *)
        echo -e "${RED}Unknown command: $COMMAND${NC}" >&2
        usage
        ;;
esac
