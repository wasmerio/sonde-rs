#!/usr/sbin/dtrace -s

#pragma D option quiet

BEGIN
{
        printf("[trace] Starting…\n");
}

Hello*:::you
{
        printf("[trace] who=`%s`\n", stringof(copyin(arg0, arg1)))
}

END
{
        printf("[trace] Ending…\n");
}