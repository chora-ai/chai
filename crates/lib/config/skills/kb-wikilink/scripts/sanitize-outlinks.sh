#!/bin/sh
# Sanitize outlink output: strip trailing backslashes from escaped pipes.
# Wikilinks inside table cells use \| as an escaped pipe:
#   [[conventions/general\|General]]
# The grep regex captures the trailing backslash as part of the link target.
# This script strips it so "conventions/general\" becomes "conventions/general".

sed 's/\\$//'
