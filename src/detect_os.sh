#!/bin/sh
# Detect OS/distro id or name
# Usage: detect_os.sh arg1 [path] where arg1 (mandatory) is either 'id' or 'name'

temp=""

#   Note: $i inside and outside awk refer to very different things!
case $1 in
    "id")
        if grep -q '^DISTRIB_ID=' "$2"/etc/lsb-release 2>/dev/null; then
            temp="$(awk -F= '$1 == "DISTRIB_ID" {print tolower($2)}' /etc/lsb-release)"
        elif grep -q '^ID=' "$2"/etc/os-release 2>/dev/null; then
            temp="$(. /etc/os-release && echo "${ID}")"
        else
            for file in "$2"/etc/*; do
                if [ "${file}" = "os-release" ]; then
                    continue
                elif [ "${file}" = "lsb-release" ]; then
                    continue
                elif echo "${file}" | grep -q -- "-release$" 2>/dev/null; then
                    temp="$(awk '{print tolower($1);exit}' "${file}")"
                    break
                fi
            done
        fi
        ;;
    "name")
        if grep -q '^NAME=' "$2"/etc/os-release 2>/dev/null; then
            temp="$(. "$2"/etc/os-release && echo "${NAME}")"
        elif grep -q '^DISTRIB_DESCRIPTION=' "$2"/etc/lsb-release 2>/dev/null; then
            temp="$(awk -F= '$1 == "DISTRIB_DESCRIPTION" {print tolower($2)}' "$2"/etc/lsb-release)"
        fi
        ;;
    *)
esac

#if [ -z "${temp}" ]; then
#    echo "Your operating system/distro could not be detected" >/dev/null 2>&1
#    break
#fi

echo $temp

