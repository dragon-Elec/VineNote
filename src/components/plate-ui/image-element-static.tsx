
import type { SlateElementProps } from '@udecode/plate';
import type { TCaptionElement } from '@udecode/plate-caption';
import type { TImageElement } from '@udecode/plate-media';

import { cn } from '@udecode/cn';
import { NodeApi, SlateElement } from '@udecode/plate';
import { useSelectedFile } from '@/components/notes-list/controllers/selected-file';
import { resolveRelativeAsset } from '@/utils/resolve-path';

export function ImageElementStatic({
  children,
  className,
  nodeProps,
  ...props
}: SlateElementProps) {
  const {
    align = 'center',
    caption,
    url,
    width,
  } = props.element as TImageElement &
    TCaptionElement & {
      width: number;
    };

  const selectedFile = useSelectedFile((state) => state.selectedFile);
  const filePath = selectedFile?.path || '';
  const resolvedUrl = resolveRelativeAsset(filePath, url);

  return (
    <SlateElement
      className={cn(className, 'py-2.5')}
      {...props}
      nodeProps={nodeProps}
    >
      <figure className="group relative m-0 inline-block" style={{ width }}>
        <div
          className="relative max-w-full min-w-[92px]"
          style={{ textAlign: align }}
        >
          <img
            className={cn(
              'w-full max-w-full cursor-default object-cover px-0',
              'rounded-sm'
            )}
            alt=""
            {...nodeProps}
            src={resolvedUrl}
          />
          {caption && (
            <figcaption className="mx-auto mt-2 h-[24px] max-w-full">
              {NodeApi.string(caption[0])}
            </figcaption>
          )}
        </div>
      </figure>
      {children}
    </SlateElement>
  );
}
