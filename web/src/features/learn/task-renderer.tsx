"use client";

import { useTranslations } from "next-intl";
import { useRef, useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  api,
  type AttemptRequest,
  type AttemptResponse,
  type TaskView,
} from "@/lib/api/client";

const supported = new Set(["single_choice", "multiple_choice", "short_answer"]);

export function TaskRenderer({
  task,
  locale,
  onAnswered,
}: {
  task: TaskView;
  locale: string;
  onAnswered: () => void;
}) {
  const t = useTranslations("lesson");
  const saved = task.latest_attempt ?? null;
  const [choiceId, setChoiceId] = useState(initialChoice(saved));
  const [choiceIds, setChoiceIds] = useState(initialChoices(saved));
  const [text, setText] = useState(initialText(saved));
  const [result, setResult] = useState<AttemptResponse | null>(saved);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const idempotencyKey = useRef<string | null>(null);

  if (!supported.has(task.type)) {
    return <p className="text-sm text-muted-foreground">{t("unsupported")}</p>;
  }

  async function submit() {
    idempotencyKey.current ??= crypto.randomUUID();
    setBusy(true);
    setError(null);
    const body = answerBody(task.type, choiceId, choiceIds, text);
    try {
      const response = await api<AttemptResponse>(
        `/tasks/${task.id}/attempts?locale=${locale}`,
        {
          method: "POST",
          csrf: true,
          idempotencyKey: idempotencyKey.current,
          body,
        },
      );
      idempotencyKey.current = null;
      setResult(response);
      onAnswered();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : t("unsupported"));
    } finally {
      setBusy(false);
    }
  }

  const correctIds = result?.correct_choice_ids ?? [];

  return (
    <div className="flex flex-col gap-3">
      <p>{task.prompt}</p>
      {task.type === "short_answer" ? (
        <div className="flex flex-col gap-2">
          <Label htmlFor={`answer-${task.id}`}>{t("answer")}</Label>
          <Input
            id={`answer-${task.id}`}
            value={text}
            onChange={(event) => setText(event.target.value)}
          />
        </div>
      ) : (
        <ul className="flex flex-col gap-2">
          {(task.choices ?? []).map((choice) => {
            const selected =
              task.type === "single_choice"
                ? choiceId === choice.id
                : choiceIds.includes(choice.id);
            const revealed = correctIds.includes(choice.id);
            return (
              <li key={choice.id}>
                <label className="flex items-center gap-2">
                  <input
                    type={task.type === "single_choice" ? "radio" : "checkbox"}
                    name={task.id}
                    checked={selected}
                    onChange={() => {
                      if (task.type === "single_choice") {
                        setChoiceId(choice.id);
                      } else {
                        setChoiceIds((current) =>
                          current.includes(choice.id)
                            ? current.filter((id) => id !== choice.id)
                            : [...current, choice.id],
                        );
                      }
                    }}
                  />
                  <span>
                    {choice.text}
                    {revealed ? " ✓" : ""}
                  </span>
                </label>
              </li>
            );
          })}
        </ul>
      )}
      <Button type="button" disabled={busy} onClick={() => void submit()}>
        {t("submit")}
      </Button>
      {result ? (
        <p className={result.correct ? "text-sm" : "text-sm text-destructive"}>
          {result.correct ? t("correct") : t("incorrect")}. {result.feedback}
        </p>
      ) : null}
      {error ? <p className="text-sm text-destructive">{error}</p> : null}
    </div>
  );
}

function answerBody(
  type: string,
  choiceId: string,
  choiceIds: string[],
  text: string,
): AttemptRequest {
  if (type === "single_choice") {
    return { type, choice_id: choiceId };
  }
  if (type === "multiple_choice") {
    return { type, choice_ids: choiceIds };
  }
  return { type, text };
}

function initialChoice(attempt: AttemptResponse | null): string {
  const submitted = attempt?.submitted_answer;
  if (submitted && typeof submitted === "object" && "choice_id" in submitted) {
    const value = submitted.choice_id;
    return typeof value === "string" ? value : "";
  }
  return "";
}

function initialChoices(attempt: AttemptResponse | null): string[] {
  const submitted = attempt?.submitted_answer;
  if (submitted && typeof submitted === "object" && "choice_ids" in submitted) {
    const value = submitted.choice_ids;
    return Array.isArray(value)
      ? value.filter((item) => typeof item === "string")
      : [];
  }
  return [];
}

function initialText(attempt: AttemptResponse | null): string {
  const submitted = attempt?.submitted_answer;
  if (submitted && typeof submitted === "object" && "text" in submitted) {
    const value = submitted.text;
    return typeof value === "string" ? value : "";
  }
  return "";
}
